# Stop test executables that have run too long, and say which tests they were in.
#
# scripts/verify.sh calls this every 10 s while `cargo test` runs. -List names a
# file with one test executable per line (cargo's own paths, from
# `cargo test --no-run --message-format=json-render-diagnostics`); -Root is the
# Windows process id of that `cargo test`. Only a process running one of those
# exact files, started by that cargo (its parent, or a parent's parent), and
# started more than -Limit seconds ago is touched: another run's tests are not.
# libtest names each test's thread after the test, so the thread names show
# which tests were still running. Exit 0: nothing over the limit. Exit 3:
# stopped at least one. Anything else: the watch itself failed.
#
#   powershell -NoProfile -File scripts/hung-tests.ps1 -List <file> -Root <pid> -Limit 300

param(
    [Parameter(Mandatory)][string]$List,
    [Parameter(Mandatory)][int]$Root,
    [int]$Limit = 300
)
$ErrorActionPreference = 'Stop'

$exes = @{}
foreach ($line in Get-Content -LiteralPath $List) {
    if ($line.Trim()) { $exes[[IO.Path]::GetFullPath($line.Trim()).ToLowerInvariant()] = $true }
}

$over = @()
foreach ($p in Get-Process) {
    try {
        $path = $p.Path
        if (-not $path) { continue }
        if (-not $exes.ContainsKey([IO.Path]::GetFullPath($path).ToLowerInvariant())) { continue }
        if (((Get-Date) - $p.StartTime).TotalSeconds -gt $Limit) { $over += $p }
    } catch {
        # A process that cannot be read (another user's, or one that just
        # ended) is not one of ours.
        continue
    }
}
# The common case: nothing over the limit. Return before the slower work below,
# since verify.sh runs this every 10 s.
if (-not $over) { exit 0 }

# Keep only the ones this run's cargo started (rustup's proxy sits between).
$parent = @{}
foreach ($w in Get-CimInstance Win32_Process) { $parent[[int]$w.ProcessId] = [int]$w.ParentProcessId }
$hung = @()
foreach ($p in $over) {
    $id = $p.Id
    for ($i = 0; $i -lt 4 -and $parent.ContainsKey($id); $i++) {
        $id = $parent[$id]
        if ($id -eq $Root) { $hung += $p; break }
    }
}
if (-not $hung) { exit 0 }

$named = $true
try {
    if (-not ('KuraHungTests' -as [type])) {
        Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class KuraHungTests {
    [DllImport("kernel32.dll", SetLastError = true)]
    static extern IntPtr OpenThread(uint access, bool inherit, uint id);
    [DllImport("kernel32.dll")]
    static extern int GetThreadDescription(IntPtr thread, out IntPtr desc);
    [DllImport("kernel32.dll")]
    static extern IntPtr LocalFree(IntPtr mem);
    [DllImport("kernel32.dll")]
    static extern bool CloseHandle(IntPtr h);
    public static string Name(uint tid) {
        IntPtr h = OpenThread(0x0800, false, tid);
        if (h == IntPtr.Zero) return "";
        try {
            IntPtr p;
            if (GetThreadDescription(h, out p) < 0 || p == IntPtr.Zero) return "";
            try { return Marshal.PtrToStringUni(p); } finally { LocalFree(p); }
        } finally { CloseHandle(h); }
    }
}
'@
    }
} catch {
    $named = $false
    Write-Output ("hung-tests: no thread names: {0}" -f $_)
}

$stopped = 0
foreach ($p in $hung) {
    try {
        $secs = [int]((Get-Date) - $p.StartTime).TotalSeconds
        $cpu = [int]$p.TotalProcessorTime.TotalSeconds
        Write-Output ("hung: {0} pid {1}, running {2} s, cpu {3} s" -f (Split-Path -Leaf $p.Path), $p.Id, $secs, $cpu)
    } catch {
        Write-Output ("hung: pid {0} ({1})" -f $p.Id, $_)
    }
    # The names are only a help: a failure to read them must not keep the
    # process alive.
    if ($named) {
        try {
            # Test threads first (libtest names them `module::test`), then the
            # other named threads once each: every test App has its own workers.
            $others = [Collections.Generic.SortedSet[string]]::new()
            foreach ($t in $p.Threads) {
                $n = [KuraHungTests]::Name([uint32]$t.Id)
                if (-not $n -or $n -eq 'main') { continue }
                if ($n.Contains('::')) { Write-Output ("  test: {0}" -f $n) } else { [void]$others.Add($n) }
            }
            if ($others.Count) { Write-Output ("  other threads: {0}" -f ($others -join ', ')) }
        } catch {
            Write-Output ("hung-tests: thread names: {0}" -f $_)
        }
    }
    try {
        Stop-Process -Id $p.Id -Force
        $stopped++
    } catch {
        Write-Output ("hung-tests: could not stop pid {0}: {1}" -f $p.Id, $_)
    }
}
if ($stopped) { exit 3 }
exit 4
