"""Windows desktop integration test; run with no other meter active.

python scripts/test-independent-start.py target/package-build/release/codex-titlebar-meter.exe
Reproduces terminal job termination without closing the user's Codex window.
Uses only Python's standard library. Cleans up the test meter after each case.
"""

import ctypes as c
from ctypes import wintypes as w
from pathlib import Path
import subprocess
import sys
import tempfile
import time

k = c.WinDLL('kernel32', use_last_error=True)
u = c.WinDLL('user32', use_last_error=True)


class BasicLimits(c.Structure):
    _fields_ = [('process_time', c.c_int64), ('job_time', c.c_int64),
                ('flags', w.DWORD), ('minimum_ws', c.c_size_t),
                ('maximum_ws', c.c_size_t), ('active_processes', w.DWORD),
                ('affinity', c.c_size_t), ('priority', w.DWORD),
                ('scheduling', w.DWORD)]


class ExtendedLimits(c.Structure):
    _fields_ = [('basic', BasicLimits), ('io', c.c_uint64 * 6),
                ('process_memory', c.c_size_t), ('job_memory', c.c_size_t),
                ('peak_process', c.c_size_t), ('peak_job', c.c_size_t)]


k.CreateJobObjectW.argtypes = [c.c_void_p, w.LPCWSTR]
k.CreateJobObjectW.restype = w.HANDLE
k.SetInformationJobObject.argtypes = [w.HANDLE, c.c_int, c.c_void_p, w.DWORD]
k.AssignProcessToJobObject.argtypes = [w.HANDLE, w.HANDLE]
k.TerminateJobObject.argtypes = [w.HANDLE, w.UINT]
k.OpenProcess.argtypes = [w.DWORD, w.BOOL, w.DWORD]
k.OpenProcess.restype = w.HANDLE
k.WaitForSingleObject.argtypes = [w.HANDLE, w.DWORD]
k.TerminateProcess.argtypes = [w.HANDLE, w.UINT]
k.CloseHandle.argtypes = [w.HANDLE]
u.FindWindowExW.argtypes = [w.HWND, w.HWND, w.LPCWSTR, w.LPCWSTR]
u.FindWindowExW.restype = w.HWND
u.GetWindowThreadProcessId.argtypes = [w.HWND, c.POINTER(w.DWORD)]


def watcher_pid():
    window = u.FindWindowExW(w.HWND(-3), None,
                            'ConfigCrate.CodexTitlebarMeter.Watcher', None)
    pid = w.DWORD()
    if window:
        u.GetWindowThreadProcessId(window, c.byref(pid))
    return pid.value


def check(ok):
    if not ok:
        raise c.WinError(c.get_last_error())


def run_case(harness, executable, installer, mode):
    job = k.CreateJobObjectW(None, None)
    check(job)
    limits = ExtendedLimits()
    limits.basic.flags = 0x2000  # JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
    launcher = None
    meter = None
    try:
        check(k.SetInformationJobObject(job, 9, c.byref(limits), c.sizeof(limits)))
        launcher = subprocess.Popen(
            ['powershell.exe', '-NoProfile', '-ExecutionPolicy', 'Bypass',
             '-File', str(harness), str(installer), str(executable), mode],
            stdin=subprocess.PIPE, stdout=subprocess.DEVNULL,
            creationflags=0x08000000)
        check(k.AssignProcessToJobObject(job, w.HANDLE(int(launcher._handle))))
        launcher.stdin.write(b'go\n')
        launcher.stdin.flush()
        deadline = time.monotonic() + 15
        pid = 0
        while time.monotonic() < deadline:
            pid = watcher_pid()
            if pid:
                break
            if launcher.poll() is not None:
                raise RuntimeError(f'{mode}: launcher exited {launcher.returncode}')
            time.sleep(0.1)
        assert pid, f'{mode}: watcher did not start'
        meter = k.OpenProcess(0x100001, False, pid)  # SYNCHRONIZE | TERMINATE
        check(meter)
        check(k.TerminateJobObject(job, 73))
        launcher.wait(timeout=5)
        survived = k.WaitForSingleObject(meter, 1500) == 258  # WAIT_TIMEOUT
        assert survived == (mode == 'fixed'), f'{mode}: survived={survived}'
        print(f'{mode}: meter survived terminal job termination = {survived}')
    finally:
        if meter:
            k.TerminateProcess(meter, 0)
            k.WaitForSingleObject(meter, 5000)
            k.CloseHandle(meter)
        k.TerminateJobObject(job, 73)
        k.CloseHandle(job)
        if launcher:
            launcher.wait(timeout=5)
            launcher.stdin.close()


if __name__ == '__main__':
    assert not watcher_pid(), 'Exit the existing meter before running this test'
    executable = Path(sys.argv[1]).resolve(strict=True)
    root = Path(__file__).resolve().parent.parent
    with tempfile.TemporaryDirectory(prefix='launch-test-', dir=root / 'target') as directory:
        harness = Path(directory) / 'launch.ps1'
        harness.write_text('''param($Installer, $Executable, $Mode)
$ErrorActionPreference = 'Stop'
$tokens = $null; $errors = $null
$ast = [System.Management.Automation.Language.Parser]::ParseFile($Installer, [ref]$tokens, [ref]$errors)
if ($errors.Count) { throw 'Installer has parse errors' }
$function = $ast.Find({ param($node) $node -is [System.Management.Automation.Language.FunctionDefinitionAst] -and $node.Name -eq 'Start-IndependentMeter' }, $true)
if (-not $function) { throw 'Independent launcher function missing' }
Invoke-Expression $function.Extent.Text
$null = [Console]::ReadLine()
if ($Mode -eq 'fixed') { Start-IndependentMeter -ExecutablePath $Executable }
else { Start-Process -FilePath $Executable -WindowStyle Hidden }
$null = [Console]::ReadLine()
''', encoding='utf-8-sig')
        for mode in ['original', 'fixed']:
            run_case(harness, executable, root / 'install.ps1', mode)
