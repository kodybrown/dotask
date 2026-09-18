/* DoTask's Windows launcher. MIT licensed; see ../../LICENSE.md.
 * Freestanding Win32 code: no CRT, SDK headers, or installed .NET runtime.
 * Only the stable Win32 declarations used below are needed to cross-compile.
 */
typedef unsigned long DWORD;
typedef unsigned short WCHAR;
typedef int BOOL;
typedef void *HANDLE;
typedef __SIZE_TYPE__ SIZE_T;
#define API __declspec(dllimport)
#define NULL ((void *)0)
#define INVALID_HANDLE ((HANDLE)(SIZE_T)-1)
typedef struct {
  DWORD cb;
  WCHAR *reserved, *desktop, *title;
  DWORD x, y, width, height, charsX, charsY, fill, flags;
  unsigned short show, reservedSize;
  unsigned char *reservedBytes;
  HANDLE input, output, error;
} STARTUPINFO;
typedef struct { HANDLE process, thread; DWORD processId, threadId; } PROCESSINFO;
API DWORD GetModuleFileNameW(HANDLE, WCHAR *, DWORD);
API WCHAR *GetCommandLineW(void);
API HANDLE GetProcessHeap(void);
API void *HeapAlloc(HANDLE, DWORD, SIZE_T);
API HANDLE CreateFileW(const WCHAR *, DWORD, DWORD, void *, DWORD, DWORD, HANDLE);
API DWORD GetFileSize(HANDLE, DWORD *);
API BOOL ReadFile(HANDLE, void *, DWORD, DWORD *, void *);
API BOOL WriteFile(HANDLE, const void *, DWORD, DWORD *, void *);
API BOOL CloseHandle(HANDLE);
API int MultiByteToWideChar(unsigned int, DWORD, const char *, int, WCHAR *, int);
API BOOL CreateProcessW(const WCHAR *, WCHAR *, void *, void *, BOOL, DWORD, void *, const WCHAR *, STARTUPINFO *, PROCESSINFO *);
API HANDLE GetStdHandle(DWORD);
API BOOL SetConsoleCtrlHandler(BOOL (*)(DWORD), BOOL);
API DWORD WaitForSingleObject(HANDLE, DWORD);
API BOOL GetExitCodeProcess(HANDLE, DWORD *);
API DWORD GetLastError(void);
API void ExitProcess(DWORD);

static void fail(const char *message) {
  DWORD length = 0, written;
  while (message[length]) ++length;
  WriteFile(GetStdHandle((DWORD)-12), message, length, &written, NULL);
  ExitProcess(1);
}
static void *allocate(SIZE_T size) {
  void *memory = HeapAlloc(GetProcessHeap(), 8 /* HEAP_ZERO_MEMORY */, size);
  if (!memory) fail("dotask shim: out of memory.\r\n");
  return memory;
}
static SIZE_T length(const WCHAR *text) {
  SIZE_T count = 0;
  while (text[count]) ++count;
  return count;
}
static BOOL control(DWORD event) {
  /* Console events also reach the child. Keep waiting for its exit code.
   * A handler, unlike SetConsoleCtrlHandler(NULL, TRUE), is not inherited.
   */
  return event == 0 || event == 1;
}
void mainCRTStartup(void) {
  WCHAR *module = allocate(32768 * sizeof(WCHAR));
  DWORD moduleLength = GetModuleFileNameW(NULL, module, 32768);
  if (moduleLength < 5 || moduleLength >= 32764 || module[moduleLength - 4] != '.')
    fail("dotask shim: cannot locate launcher.\r\n");
  WCHAR *sidecar = allocate(32768 * sizeof(WCHAR));
  for (DWORD i = 0; i < moduleLength - 3; ++i) sidecar[i] = module[i];
  sidecar[moduleLength - 3] = 's'; sidecar[moduleLength - 2] = 'h';
  sidecar[moduleLength - 1] = 'i'; sidecar[moduleLength] = 'm';
  HANDLE file = CreateFileW(sidecar, 0x80000000, 7 /* share read/write/delete */, NULL, 3, 0, NULL);
  if (file == INVALID_HANDLE) fail("dotask shim: cannot open matching .shim file.\r\n");
  DWORD size = GetFileSize(file, NULL), read;
  if (!size || size > 65536) fail("dotask shim: invalid .shim file size.\r\n");
  char *bytes = allocate(size + 1);
  if (!ReadFile(file, bytes, size, &read, NULL) || read != size)
    fail("dotask shim: cannot read .shim file.\r\n");
  CloseHandle(file);
  WCHAR *config = allocate((size + 1) * sizeof(WCHAR));
  int count = MultiByteToWideChar(65001 /* UTF-8 */, 8 /* reject invalid UTF-8 */, bytes, size, config, size);
  if (count < 10) fail("dotask shim: .shim file must contain a UTF-8 path assignment.\r\n");
  WCHAR *p = config;
  if (*p == 0xfeff) ++p;
  if (p[0] != 'p' || p[1] != 'a' || p[2] != 't' || p[3] != 'h')
    fail("dotask shim: expected path = \"absolute executable path\".\r\n");
  p += 4;
  while (*p == ' ' || *p == '\t') ++p;
  if (*p++ != '=') fail("dotask shim: missing path assignment.\r\n");
  while (*p == ' ' || *p == '\t') ++p;
  if (*p++ != '"') fail("dotask shim: executable path must be quoted.\r\n");
  WCHAR *target = p;
  while (*p && *p != '"' && *p >= 32) ++p;
  if (*p != '"' || p == target) fail("dotask shim: invalid executable path.\r\n");
  *p++ = 0;
  while (*p == ' ' || *p == '\t' || *p == '\r' || *p == '\n') ++p;
  if (*p || p != config + count) fail("dotask shim: unsupported .shim contents.\r\n");
  SIZE_T targetLength = length(target);
  if (targetLength < 4 || !((target[1] == ':' && (target[2] == '\\' || target[2] == '/')) ||
      (target[0] == '\\' && target[1] == '\\')))
    fail("dotask shim: executable path must be absolute.\r\n");
  BOOL same = targetLength == moduleLength;
  for (SIZE_T i = 0; same && i < targetLength; ++i) {
    WCHAR a = target[i], b = module[i];
    if (a >= 'A' && a <= 'Z') a += 32;
    if (b >= 'A' && b <= 'Z') b += 32;
    if (a != b) same = 0;
  }
  if (same) fail("dotask shim: executable points to the launcher itself.\r\n");

  /* Preserve the original argument tail verbatim, including empty arguments,
   * quotes, Unicode, backslashes and shell metacharacters. No shell is used.
   */
  WCHAR *tail = GetCommandLineW();
  while (*tail == ' ' || *tail == '\t') ++tail;
  BOOL quoted = 0;
  while (*tail) {
    if (*tail == '"') quoted = !quoted;
    else if (!quoted && (*tail == ' ' || *tail == '\t')) break;
    ++tail;
  }
  SIZE_T tailLength = length(tail);
  if (targetLength + tailLength + 3 > 32767) fail("dotask shim: command line is too long.\r\n");
  WCHAR *command = allocate((targetLength + tailLength + 3) * sizeof(WCHAR));
  command[0] = '"';
  for (SIZE_T i = 0; i < targetLength; ++i) command[i + 1] = target[i];
  command[targetLength + 1] = '"';
  for (SIZE_T i = 0; i < tailLength; ++i) command[targetLength + 2 + i] = tail[i];
  STARTUPINFO *startup = allocate(sizeof(STARTUPINFO));
  PROCESSINFO *child = allocate(sizeof(PROCESSINFO));
  startup->cb = sizeof(STARTUPINFO);
  startup->flags = 0x100 /* STARTF_USESTDHANDLES */;
  startup->input = GetStdHandle((DWORD)-10);
  startup->output = GetStdHandle((DWORD)-11);
  startup->error = GetStdHandle((DWORD)-12);
  if (!SetConsoleCtrlHandler(control, 1)) fail("dotask shim: cannot register console handler.\r\n");
  if (!CreateProcessW(target, command, NULL, NULL, 1, 0, NULL, NULL, startup, child)) {
    DWORD error = GetLastError(), written, digits = 0;
    char number[16];
    do { number[digits++] = (char)('0' + error % 10); error /= 10; } while (error);
    const char *prefix = "dotask shim: cannot launch executable; Windows error ";
    DWORD prefixLength = 0;
    while (prefix[prefixLength]) ++prefixLength;
    WriteFile(GetStdHandle((DWORD)-12), prefix, prefixLength, &written, NULL);
    while (digits) WriteFile(GetStdHandle((DWORD)-12), &number[--digits], 1, &written, NULL);
    fail(".\r\n");
  }
  CloseHandle(child->thread);
  if (WaitForSingleObject(child->process, (DWORD)-1) != 0) fail("dotask shim: wait failed.\r\n");
  DWORD exitCode;
  if (!GetExitCodeProcess(child->process, &exitCode)) fail("dotask shim: cannot read exit code.\r\n");
  CloseHandle(child->process);
  ExitProcess(exitCode);
}
