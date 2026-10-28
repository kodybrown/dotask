//go:build windows

package installer

import (
	"errors"
	"fmt"
	"os/exec"
	"syscall"
	"time"
	"unsafe"
)

// Start suspended so the process cannot create an untracked descendant before
// assignment. Each invocation owns a nested kill-on-close job; a local timeout
// must stop descendants even though the outer task itself is not cancelled.
func runContained(command *exec.Cmd) error {
	kernel := syscall.NewLazyDLL("kernel32.dll")
	create := kernel.NewProc("CreateJobObjectW")
	set := kernel.NewProc("SetInformationJobObject")
	assign := kernel.NewProc("AssignProcessToJobObject")
	terminate := kernel.NewProc("TerminateJobObject")
	query := kernel.NewProc("QueryInformationJobObject")
	job, _, err := create.Call(0, 0)
	if job == 0 {
		return fmt.Errorf("create acceptance job: %w", err)
	}
	defer syscall.CloseHandle(syscall.Handle(job))
	// Native x64/arm64 layout of JOBOBJECT_EXTENDED_LIMIT_INFORMATION.
	type basicLimits struct {
		processTime, jobTime int64
		flags                uint32
		minimum, maximum     uintptr
		active               uint32
		affinity             uintptr
		priority, scheduling uint32
	}
	type limits struct {
		basic                                          basicLimits
		io                                             [6]uint64
		processMemory, jobMemory, peakProcess, peakJob uintptr
	}
	limit := limits{}
	limit.basic.flags = 0x2000 // JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
	ok, _, err := set.Call(job, 9, uintptr(unsafe.Pointer(&limit)), unsafe.Sizeof(limit))
	if ok == 0 {
		return fmt.Errorf("configure acceptance job: %w", err)
	}
	command.SysProcAttr = &syscall.SysProcAttr{CreationFlags: 0x08000004} // no window, suspended
	if err = command.Start(); err != nil {
		return err
	}
	fail := func(cause error) error { _ = command.Process.Kill(); _ = command.Wait(); return cause }
	process, err := syscall.OpenProcess(0x0101, false, uint32(command.Process.Pid))
	if err != nil {
		return fail(err)
	}
	ok, _, err = assign.Call(job, uintptr(process))
	syscall.CloseHandle(process)
	if ok == 0 {
		return fail(fmt.Errorf("assign acceptance process to job: %w", err))
	}
	if err = resumeAcceptanceProcess(kernel, uint32(command.Process.Pid)); err != nil {
		return fail(err)
	}
	outcome := command.Wait()
	ok, _, err = terminate.Call(job, 130)
	if ok == 0 {
		return errors.Join(outcome, fmt.Errorf("stop acceptance descendants: %w", err))
	}
	// Termination is asynchronous. Wait until no process can retain fixture files
	// or its working directory; a completed parent alone is insufficient.
	deadline := time.Now().Add(5 * time.Second)
	type accounting struct {
		times                             [4]int64
		faults, total, active, terminated uint32
	}
	for {
		var state accounting
		ok, _, err = query.Call(job, 1, uintptr(unsafe.Pointer(&state)), unsafe.Sizeof(state), 0)
		if ok == 0 {
			return errors.Join(outcome, fmt.Errorf("query acceptance job: %w", err))
		}
		if state.active == 0 {
			return outcome
		}
		if time.Now().After(deadline) {
			return errors.Join(outcome, errors.New("timed out stopping installer descendants"))
		}
		time.Sleep(10 * time.Millisecond)
	}
}

func resumeAcceptanceProcess(kernel *syscall.LazyDLL, pid uint32) error {
	snapshot, _, err := kernel.NewProc("CreateToolhelp32Snapshot").Call(4, 0)
	if snapshot == ^uintptr(0) {
		return err
	}
	defer syscall.CloseHandle(syscall.Handle(snapshot))
	type threadEntry struct {
		size, usage, id, owner uint32
		base, delta            int32
		flags                  uint32
	}
	entry := threadEntry{}
	entry.size = uint32(unsafe.Sizeof(entry))
	next := kernel.NewProc("Thread32Next")
	ok, _, err := kernel.NewProc("Thread32First").Call(snapshot, uintptr(unsafe.Pointer(&entry)))
	for ok != 0 {
		if entry.owner == pid {
			thread, _, openErr := kernel.NewProc("OpenThread").Call(2, 0, uintptr(entry.id))
			if thread == 0 {
				return openErr
			}
			defer syscall.CloseHandle(syscall.Handle(thread))
			result, _, resumeErr := kernel.NewProc("ResumeThread").Call(thread)
			if result == 0xffffffff {
				return resumeErr
			}
			return nil
		}
		ok, _, err = next.Call(snapshot, uintptr(unsafe.Pointer(&entry)))
	}
	return fmt.Errorf("find suspended installer thread: %w", err)
}

func persistentPaths() ([]registryValue, error) {
	read := syscall.NewLazyDLL("advapi32.dll").NewProc("RegGetValueW")
	query := func(root uintptr, key string) (registryValue, error) {
		subkey, err := syscall.UTF16PtrFromString(key)
		if err != nil {
			return registryValue{}, err
		}
		name, _ := syscall.UTF16PtrFromString("Path")
		var kind, size uint32
		// Preserve registry type and unexpanded bytes, including absence. Inherited
		// process PATH alone cannot prove persistent user/machine state was unchanged.
		status, _, _ := read.Call(root, uintptr(unsafe.Pointer(subkey)), uintptr(unsafe.Pointer(name)), 0x10000006, uintptr(unsafe.Pointer(&kind)), 0, uintptr(unsafe.Pointer(&size)))
		if status == 2 || status == 3 {
			return registryValue{}, nil
		}
		if status != 0 {
			return registryValue{}, fmt.Errorf("read persistent PATH: %w", syscall.Errno(status))
		}
		data := make([]byte, max(size, 1))
		status, _, _ = read.Call(root, uintptr(unsafe.Pointer(subkey)), uintptr(unsafe.Pointer(name)), 0x10000006, uintptr(unsafe.Pointer(&kind)), uintptr(unsafe.Pointer(&data[0])), uintptr(unsafe.Pointer(&size)))
		if status != 0 {
			return registryValue{}, fmt.Errorf("read persistent PATH bytes: %w", syscall.Errno(status))
		}
		return registryValue{present: true, kind: kind, data: data[:size]}, nil
	}
	// Predefined HKEY values are sign extended on 64-bit Windows.
	user, err := query(^uintptr(0x7ffffffe), "Environment")
	if err != nil {
		return nil, err
	}
	machine, err := query(^uintptr(0x7ffffffd), `SYSTEM\CurrentControlSet\Control\Session Manager\Environment`)
	if err != nil {
		return nil, err
	}
	return []registryValue{user, machine}, nil
}

func retryableCleanup(err error) bool {
	return errors.Is(err, syscall.Errno(5)) || errors.Is(err, syscall.Errno(32)) || errors.Is(err, syscall.Errno(33))
}
