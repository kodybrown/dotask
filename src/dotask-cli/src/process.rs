use anyhow::{Context, Result, bail};
use std::process::{Child, Command};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::sleep;
use std::time::{Duration, Instant};

static CANCELLED: AtomicBool = AtomicBool::new(false);

#[derive(Debug)]
pub(crate) struct Cancelled;
impl std::fmt::Display for Cancelled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Cancelled.")
    }
}
impl std::error::Error for Cancelled {}

pub(crate) fn initialize() -> Result<()> {
    ctrlc::set_handler(|| CANCELLED.store(true, Ordering::SeqCst))?;
    Ok(())
}

pub(crate) fn check_cancelled() -> Result<()> {
    if CANCELLED.load(Ordering::SeqCst) {
        bail!(Cancelled);
    }
    Ok(())
}

pub(crate) fn run(command: &mut Command) -> Result<i32> {
    check_cancelled()?;
    let mut process = Process::spawn(command)
        .with_context(|| format!("Cannot start {:?}", command.get_program()))?;
    loop {
        if check_cancelled().is_err() {
            // Give task cancellation handlers time to release their own resources,
            // then stop the entire owned group, including uncooperative children.
            process.interrupt();
            let until = Instant::now() + Duration::from_millis(750);
            while Instant::now() < until {
                let _ = process.child.try_wait();
                sleep(Duration::from_millis(20));
            }
            process.terminate()?;
            bail!(Cancelled);
        }
        if let Some(status) = process.child.try_wait()? {
            process.finished = true;
            #[cfg(unix)]
            {
                use std::os::unix::process::ExitStatusExt;
                return Ok(status
                    .code()
                    .unwrap_or_else(|| 128 + status.signal().unwrap_or(1)));
            }
            #[cfg(windows)]
            return Ok(status.code().unwrap_or(1));
        }
        sleep(Duration::from_millis(20));
    }
}

struct Process {
    child: Child,
    finished: bool,
    #[cfg(unix)]
    group: Option<i32>,
    #[cfg(windows)]
    job: windows_job::Job,
}

impl Process {
    fn spawn(command: &mut Command) -> Result<Self> {
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            // Nested native calls stay in the outer task's process group. If
            // each nested runner made a new group, killing its parent could
            // strand grandchildren before their own grace interval expired.
            let inherited = std::env::var("DOTASK_PROCESS_GROUP_OWNER")
                .ok()
                .and_then(|s| s.parse::<i32>().ok())
                .is_some_and(|owner| {
                    owner > 0
                        && unsafe {
                            let owner_group = libc::getpgid(owner);
                            owner_group >= 0 && owner_group != libc::getpgrp()
                        }
                });
            if !inherited {
                command
                    .process_group(0)
                    .env("DOTASK_PROCESS_GROUP_OWNER", std::process::id().to_string());
            }
            let child = command.spawn()?;
            let group = if inherited {
                None
            } else {
                Some(child.id() as i32)
            };
            Ok(Self {
                child,
                group,
                finished: false,
            })
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            use windows::Win32::System::Threading::CREATE_SUSPENDED;
            let job = windows_job::Job::new()?;
            // Assign before the first instruction can spawn an untracked child.
            let child = command.creation_flags(CREATE_SUSPENDED.0).spawn()?;
            let process = Self {
                child,
                finished: false,
                job,
            };
            process.job.assign_and_resume(&process.child)?;
            Ok(process)
        }
    }

    fn interrupt(&self) {
        // Windows console events already reach attached children. On Unix the
        // outer runner forwards to the group shared by the task and nested calls.
        #[cfg(unix)]
        if let Some(group) = self.group {
            // Only the outer owner forwards; its whole group receives this.
            unsafe {
                libc::kill(-group, libc::SIGINT);
            }
        }
    }

    fn terminate(&mut self) -> Result<()> {
        #[cfg(windows)]
        self.job.terminate()?;
        #[cfg(unix)]
        if let Some(group) = self.group {
            unsafe {
                libc::kill(-group, libc::SIGKILL);
            }
        }
        // Also handles a failure between suspended spawn and job assignment.
        let _ = self.child.kill();
        self.child.wait()?;
        self.finished = true;
        Ok(())
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        if !self.finished {
            let _ = self.terminate();
        }
    }
}

#[cfg(windows)]
mod windows_job {
    use super::*;
    use std::os::windows::io::AsRawHandle;
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First, Thread32Next,
    };
    use windows::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOBOBJECT_BASIC_ACCOUNTING_INFORMATION,
        JobObjectBasicAccountingInformation, QueryInformationJobObject, TerminateJobObject,
    };
    use windows::Win32::System::Threading::{OpenThread, ResumeThread, THREAD_SUSPEND_RESUME};

    struct Handle(HANDLE);
    impl Drop for Handle {
        fn drop(&mut self) {
            // Each handle is owned here and closed exactly once.
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
    pub(super) struct Job(Handle);
    impl Job {
        pub(super) fn new() -> Result<Self> {
            Ok(Self(Handle(unsafe { CreateJobObjectW(None, None)? })))
        }

        pub(super) fn assign_and_resume(&self, child: &Child) -> Result<()> {
            // The child is suspended and cannot exit normally or create threads.
            // Toolhelp finds its initial thread without undocumented NT APIs.
            unsafe {
                AssignProcessToJobObject(self.0.0, HANDLE(child.as_raw_handle()))?;
                let snapshot = Handle(CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0)?);
                let mut entry = THREADENTRY32 {
                    dwSize: size_of::<THREADENTRY32>() as u32,
                    ..Default::default()
                };
                Thread32First(snapshot.0, &mut entry)?;
                loop {
                    if entry.th32OwnerProcessID == child.id() {
                        let thread = Handle(OpenThread(
                            THREAD_SUSPEND_RESUME,
                            false,
                            entry.th32ThreadID,
                        )?);
                        if ResumeThread(thread.0) == u32::MAX {
                            return Err(std::io::Error::last_os_error().into());
                        }
                        return Ok(());
                    }
                    Thread32Next(snapshot.0, &mut entry)?;
                }
            }
        }

        pub(super) fn terminate(&self) -> Result<()> {
            // Wait for descendants too before removing snapshots they may hold.
            unsafe {
                TerminateJobObject(self.0.0, 130)?;
                let until = Instant::now() + Duration::from_secs(10);
                loop {
                    let mut info = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
                    QueryInformationJobObject(
                        Some(self.0.0),
                        JobObjectBasicAccountingInformation,
                        &mut info as *mut _ as *mut _,
                        size_of_val(&info) as u32,
                        None,
                    )?;
                    if info.ActiveProcesses == 0 {
                        return Ok(());
                    }
                    if Instant::now() >= until {
                        bail!("Timed out stopping the task process tree.");
                    }
                    sleep(Duration::from_millis(20));
                }
            }
        }
    }
}
