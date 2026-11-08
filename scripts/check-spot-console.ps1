<#
Measure TESTING rows 46.11 and 46.11a (Windows): spot on files in a Git repository
must not show a console window and must not leave a `conhost.exe` per `git.exe`
(v0.150.1 starts `git.exe` with DETACHED_PROCESS; CREATE_NO_WINDOW only hid the
window and each `git.exe` still got a `conhost`: 21 after six spots, #263).

  scripts\check-spot-console.ps1 [-Out C:\dev\evidence\spot-console] [-Kura path\kura.exe] [-Timeout 120] [-Interval 5]
  scripts\check-spot-console.ps1 -SelfTest [-Out ...]

Builds a Git repository in `<Out>\run-NN\repo` (six committed files with two commits
each, one of them modified, and one untracked), starts the watcher, then runs
kura (a release build: a debug build owns a console, src/main.rs:1) with `--keys`:
`<Tab>` on each of the six files, `<A-j>` down to the Git rows, `<State:>` to read
`spot row:`. The run has its own folder, KURA_KEYS_DONE, and every variable of
scripts\lanes.conf's `isolate=` points at an empty folder in it, so the machine's
config is not read or written. The watcher stops after kura has exited.

kura runs with the machine's and the user's PATH, the one a normal launch has, not the
PATH of the shell that started this script. Git Bash puts `mingw64\bin` first and finds
git's own `git.exe`; a normal launch finds `Git\cmd\git.exe`, a wrapper that starts the
real one. With DETACHED_PROCESS the wrapper has no console, so Windows gives the inner
`git.exe` one of its own: the two PATHs give different answers for 46.11a, so kura starts
git's own exe, found once through `git --exec-path` (v0.159.2, #519).

The watcher (C# through Add-Type, started BEFORE kura, stopped AFTER it):
  - a thread with a message pump and SetWinEventHook(EVENT_OBJECT_CREATE ..
    EVENT_OBJECT_SHOW, WINEVENT_OUTOFCONTEXT) for OBJID_WINDOW: time, hwnd,
    class, title, visibility, owning pid of every window of a console class
    (ConsoleWindowClass, CASCADIA_HOSTING_WINDOW_CLASS, PseudoConsoleWindow).
    A SHOW event counts as visible even when the window is gone by the time
    the callback runs. No sample can miss it: the system queues the events.
  - a process-table loop (NtQuerySystemInformation, -Interval ms of sleep between reads, default 5) that records every process
    that appears (pid, name, parent pid, creation time, first/last seen), also the ones that
    have gone since. The tree is worked out afterwards from the parent chain.
Attribution: a console window belongs to kura when its owning pid is in kura's
tree or the parent of that pid is (a console window's thread belongs to the
`conhost.exe`, whose parent is the process that created the console). With
Windows Terminal as the default terminal the window (class
CASCADIA_HOSTING_WINDOW_CLASS) belongs to WindowsTerminal.exe: it is kura's when a
console host under kura's tree appeared within 1.5 s before to 0.5 s after it. A conhost
counts as under kura when its parent is kura or one of kura's descendants.

Checks (a line each; the last line is `check-spot-console: ok`, exit 0, only if all hold):
  measured   at least one `git.exe` was seen under kura (else the run proves nothing)
  46.11      no visible console-class window attributed to kura's tree appeared
  46.11a     no conhost.exe / OpenConsole.exe seen under kura's tree but the one of the
             first git directly under kura: ito_proc::git_program() asks `git --exec-path`
             once per process, through the wrapper when that is what PATH finds, and
             that one call owns a console host (#519). A second host fails. (The
             machine's conhost count before and after is printed as information only.)
  git        the Git section is there: with the cursor on row 11 `spot row:` is `Last change`,
             on row 14 it is `Commits`, for all six files
  keys       the run ended `keys: done`
  events     the window hook saw at least one window event (kura's own window makes some)
When all of those hold, a console window that appeared and is not attributable to kura
is not a pass: `check-spot-console: inconclusive`, exit 3, with what appeared. Re-run.
Exit 1: a check failed. Exit 2: the setup failed (no kura, no git, no 64-bit PowerShell,
the test repository could not be built, the watcher could not start).

NOT covered: the status marks in the list and the branch name (no --keys reading
shows them: `<State:>` has no git lines), a Windows Terminal that has taken over
consoles when the window is not found by class, a process that lives for less than
one snapshot cycle (about 13 ms here: 5 ms of sleep and 8 ms of reading), a window
that has gone before the callback reads its class (the watcher prints how many SHOW
events that was), and what happens on ARM64.

-SelfTest proves the checks can fail, without kura: a helper root process in place of kura,
watched the same way: a visible console must fail 46.11; CREATE_NO_WINDOW must fail
46.11a and pass 46.11; DETACHED_PROCESS on git's own `git.exe` (not the wrapper in
`Git\cmd`) must pass both; a detached helper whose one git opens a console must pass
46.11a (the allowance for the first git) and one whose two gits each open a console
must fail it; no process at all must fail "measured". Exit 0 only when all six came
out as expected. The two detached helpers put console windows on the screen for a moment.
#>
param(
    [string]$Out,
    [string]$Kura,
    [int]$Timeout = 120,
    [int]$Interval = 5,
    [switch]$SelfTest
)
$ErrorActionPreference = 'Stop'
if ([IntPtr]::Size -ne 8) { Write-Output 'check-spot-console: needs a 64-bit PowerShell (the process-table offsets are the x64 and ARM64 ones)'; exit 2 }
$repo = Split-Path $PSScriptRoot -Parent
if (-not $Out) { $Out = Join-Path $env:TEMP 'check-spot-console' }
New-Item -ItemType Directory -Force -Path $Out | Out-Null
$Out = (Resolve-Path -LiteralPath $Out).Path
if (-not (Get-Command git.exe -ErrorAction SilentlyContinue)) { Write-Output 'check-spot-console: git.exe is not on PATH'; exit 2 }

Add-Type -Language CSharp -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading;

public class WatchProc { public int Pid, Ppid; public string Name; public long Created, Seen, Last; }
public class WatchWin { public long T; public string Ev, Class, Title; public long Hwnd; public bool Visible; public int Pid; }

public static class SpotWatch {
    delegate void WinEventProc(IntPtr hook, uint ev, IntPtr hwnd, int idObject, int idChild, uint thread, uint time);
    [DllImport("user32.dll")] static extern IntPtr SetWinEventHook(uint min, uint max, IntPtr mod, WinEventProc cb, uint pid, uint tid, uint flags);
    [DllImport("user32.dll")] static extern bool UnhookWinEvent(IntPtr hook);
    [DllImport("user32.dll")] static extern int GetMessage(out MSG m, IntPtr hwnd, uint min, uint max);
    [DllImport("user32.dll")] static extern bool PostThreadMessage(uint tid, uint msg, IntPtr w, IntPtr l);
    [DllImport("user32.dll")] static extern bool TranslateMessage(ref MSG m);
    [DllImport("user32.dll")] static extern IntPtr DispatchMessage(ref MSG m);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetClassName(IntPtr h, StringBuilder sb, int n);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetWindowText(IntPtr h, StringBuilder sb, int n);
    [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("kernel32.dll")] static extern uint GetCurrentThreadId();
    [DllImport("ntdll.dll")] static extern int NtQuerySystemInformation(int cls, IntPtr buf, int len, out int ret);

    [StructLayout(LayoutKind.Sequential)] struct MSG { public IntPtr hwnd; public uint message; public IntPtr wParam, lParam; public uint time; public int x, y; }
    static readonly Stopwatch clock = new Stopwatch();
    static readonly object gate = new object();
    static WinEventProc keep;
    static Thread pump, sampler;
    static uint pumpTid;
    static volatile bool stop, hooked;
    static IntPtr hook;
    public static List<WatchProc> Procs = new List<WatchProc>();
    public static List<WatchWin> Wins = new List<WatchWin>();
    public static long Loops, WinEvents, Gone, GoneShown;
    public static double CycleTotalMs, CycleMaxMs;

    static bool ConsoleClass(string c) {
        return c == "ConsoleWindowClass" || c == "CASCADIA_HOSTING_WINDOW_CLASS" || c == "PseudoConsoleWindow";
    }

    static void OnEvent(IntPtr h, uint ev, IntPtr hwnd, int idObject, int idChild, uint thread, uint time) {
        if (idObject != 0 || idChild != 0 || hwnd == IntPtr.Zero) return;
        long t = clock.ElapsedMilliseconds;
        Interlocked.Increment(ref WinEvents);
        var cls = new StringBuilder(256);
        if (GetClassName(hwnd, cls, 256) == 0) {
            // Any window on the desktop, not only a console: printed, not judged.
            Interlocked.Increment(ref Gone);
            if (ev == 0x8002) Interlocked.Increment(ref GoneShown);
            return;
        }
        string c = cls.ToString();
        if (!ConsoleClass(c)) return;
        var ttl = new StringBuilder(256);
        GetWindowText(hwnd, ttl, 256);
        uint pid; GetWindowThreadProcessId(hwnd, out pid);
        var w = new WatchWin { T = t, Ev = ev == 0x8002 ? "show" : "create", Class = c, Title = ttl.ToString(), Hwnd = hwnd.ToInt64(), Pid = (int)pid };
        w.Visible = ev == 0x8002 || IsWindowVisible(hwnd);
        lock (gate) Wins.Add(w);
    }

    static void Pump() {
        pumpTid = GetCurrentThreadId();
        keep = OnEvent;
        // EVENT_OBJECT_CREATE = 0x8000 .. EVENT_OBJECT_SHOW = 0x8002; WINEVENT_OUTOFCONTEXT = 0
        hook = SetWinEventHook(0x8000, 0x8002, IntPtr.Zero, keep, 0, 0, 0);
        hooked = true;
        MSG m;
        while (GetMessage(out m, IntPtr.Zero, 0, 0) > 0) { TranslateMessage(ref m); DispatchMessage(ref m); }
        if (hook != IntPtr.Zero) UnhookWinEvent(hook);
    }

    // SystemProcessInformation (class 5) in one call into a buffer reused every time: the Toolhelp snapshot allocated a string per
    // process and took 12 ms on this machine. x64/ARM64 layout: NextEntryOffset @0, CreateTime @32, ImageName (Length @56, Buffer @64),
    // UniqueProcessId @80, InheritedFromUniqueProcessId @88. The name is only read for a pid not seen before.
    static void Sample(int interval) {
        var live = new Dictionary<int, WatchProc>();
        var sw = new Stopwatch();
        int size = 1 << 21;
        IntPtr buf = Marshal.AllocHGlobal(size);
        while (!stop) {
            sw.Restart();
            int ret;
            int st = NtQuerySystemInformation(5, buf, size, out ret);
            while (st == unchecked((int)0xC0000004)) { // STATUS_INFO_LENGTH_MISMATCH
                Marshal.FreeHGlobal(buf); size = ret + (1 << 16); buf = Marshal.AllocHGlobal(size);
                st = NtQuerySystemInformation(5, buf, size, out ret);
            }
            if (st >= 0) {
                var now = new Dictionary<int, WatchProc>();
                long t = clock.ElapsedMilliseconds;
                long off = 0;
                while (true) {
                    IntPtr e = new IntPtr(buf.ToInt64() + off);
                    int pid = (int)Marshal.ReadIntPtr(e, 80).ToInt64();
                    int ppid = (int)Marshal.ReadIntPtr(e, 88).ToInt64();
                    long created = Marshal.ReadInt64(e, 32);
                    if (pid != 0) {
                        WatchProc p;
                        if (live.TryGetValue(pid, out p) && p.Created == created) { p.Last = t; }
                        else {
                            int len = Marshal.ReadInt16(e, 56) / 2;
                            string name = len > 0 ? Marshal.PtrToStringUni(Marshal.ReadIntPtr(e, 64), len) : "";
                            p = new WatchProc { Pid = pid, Ppid = ppid, Name = name, Created = created, Seen = t, Last = t };
                            lock (gate) Procs.Add(p);
                        }
                        now[pid] = p;
                    }
                    int next = Marshal.ReadInt32(e, 0);
                    if (next == 0) break;
                    off += next;
                }
                live = now;
            }
            Loops++;
            double ms = sw.Elapsed.TotalMilliseconds;
            CycleTotalMs += ms; if (ms > CycleMaxMs) CycleMaxMs = ms;
            Thread.Sleep(interval);
        }
        Marshal.FreeHGlobal(buf);
    }

    public static void Start(int interval) {
        stop = false; hooked = false; clock.Restart();
        Procs = new List<WatchProc>(); Wins = new List<WatchWin>();
        Loops = WinEvents = Gone = GoneShown = 0; CycleTotalMs = CycleMaxMs = 0;
        pump = new Thread(Pump); pump.IsBackground = true; pump.Start();
        sampler = new Thread(delegate() { Sample(interval); }); sampler.IsBackground = true; sampler.Start();
        // The hook is in place and the first snapshot taken before anything is started.
        var waited = Stopwatch.StartNew();
        while (!hooked || Loops < 2) {
            if (waited.ElapsedMilliseconds > 10000) throw new TimeoutException("the watcher did not start within 10 s");
            Thread.Sleep(1);
        }
        if (hook == IntPtr.Zero) throw new InvalidOperationException("SetWinEventHook failed");
        Thread.Sleep(50);
    }
    public static long Now() { return clock.ElapsedMilliseconds; }
    public static void Stop() {
        Thread.Sleep(300); // events and a last snapshot for what the root left behind
        stop = true; sampler.Join();
        PostThreadMessage(pumpTid, 0x0012, IntPtr.Zero, IntPtr.Zero); // WM_QUIT
        pump.Join();
    }
}

public static class Spawn {
    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    struct SI { public int cb; public string r, desk, title; public int x, y, xs, ys, xc, yc, fill, flags; public short show, res2; public IntPtr res2p, hin, hout, herr; }
    [StructLayout(LayoutKind.Sequential)] struct PI { public IntPtr hp, ht; public uint pid, tid; }
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    static extern bool CreateProcessW(string app, string cmd, IntPtr pa, IntPtr ta, bool inherit, uint flags, IntPtr env, string dir, ref SI si, out PI pi);
    [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr h);
    [DllImport("kernel32.dll")] static extern uint WaitForSingleObject(IntPtr h, uint ms);
    [DllImport("kernel32.dll")] static extern bool TerminateProcess(IntPtr h, uint code);
    // Starts a process with the given creation flags and no standard handles passed on. Returns its pid and a handle.
    public static IntPtr Last;
    public static int Start(string cmdline, uint flags, string dir) {
        var si = new SI(); si.cb = Marshal.SizeOf(typeof(SI)); si.flags = 0x1; si.show = 5; // STARTF_USESHOWWINDOW, SW_SHOW
        PI pi;
        if (!CreateProcessW(null, cmdline, IntPtr.Zero, IntPtr.Zero, false, flags, IntPtr.Zero, dir, ref si, out pi))
            throw new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error());
        CloseHandle(pi.ht); Last = pi.hp; return (int)pi.pid;
    }
    public static bool Wait(int ms) { return WaitForSingleObject(Last, (uint)ms) == 0; }
    public static void Kill() { TerminateProcess(Last, 1); }
}
'@

$consoleHosts = 'conhost.exe', 'OpenConsole.exe'

function Get-Tree([int]$Root, $Procs) {
    # Every pid under $Root by the parent chain, the root included. A record is taken in when its
    # parent is in the set, repeated until nothing is added.
    $set = New-Object 'System.Collections.Generic.HashSet[int]'
    if ($Root -le 0) { return , $set }  # no root process: an empty tree
    [void]$set.Add($Root)
    do {
        $n = $set.Count
        foreach ($p in $Procs) { if ($set.Contains($p.Ppid)) { [void]$set.Add($p.Pid) } }
    } while ($set.Count -ne $n)
    , $set
}

# Judge one watched run. $Root is the root process' pid (0 for none). Returns the verdicts as a hashtable and the
# lines that explain them.
function Get-Verdict([int]$Root, $Procs, $Wins) {
    $v = [ordered]@{}
    $why = New-Object System.Collections.Generic.List[string]
    $tree = Get-Tree $Root $Procs
    $under = @($Procs | Where-Object { $tree.Contains($_.Pid) })
    $gits = @($under | Where-Object { $_.Name -ieq 'git.exe' })
    $hosts = @($under | Where-Object { $consoleHosts -contains $_.Name -and $_.Pid -ne $Root })
    $byPid = @{}; foreach ($p in $Procs) { $byPid[$p.Pid] = $p }
    $mine = @(); $foreign = @()
    foreach ($w in @($Wins | Where-Object { $_.Visible -and $_.Class -ne 'PseudoConsoleWindow' })) {
        $own = $w.Pid
        $par = if ($byPid.ContainsKey($own)) { $byPid[$own].Ppid } else { -1 }
        # Windows Terminal as the default terminal owns the window (its pid is not in the tree): there the tree's
        # console host (conhost / OpenConsole under the tree) that appeared within 1.5 s before to 0.5 s after the window is the link.
        $near = $w.Class -eq 'CASCADIA_HOSTING_WINDOW_CLASS' -and @($hosts | Where-Object { ($w.T - $_.Seen) -le 1500 -and ($_.Seen - $w.T) -le 500 }).Count -gt 0
        if ($tree.Contains($own) -or $tree.Contains($par) -or $near) { $mine += $w } else { $foreign += $w }
    }
    $v.measured = ($gits.Count -ge 1)
    $v.p4611 = ($mine.Count -eq 0)
    # ito_proc::git_program() asks `git --exec-path` once per process, through PATH's wrapper when that is what is
    # found, and that one call owns a console host (#519). It is the first git directly under the root: its host is allowed, no other.
    $firstGit = @($gits | Where-Object { $_.Ppid -eq $Root } | Sort-Object Seen | Select-Object -First 1)
    $probe = if ($firstGit.Count) { $firstGit[0].Pid } else { -1 }
    $probeHosts = @($hosts | Where-Object { $_.Ppid -eq $probe })
    $otherHosts = @($hosts | Where-Object { $_.Ppid -ne $probe })
    $v.p4611a = ($otherHosts.Count -eq 0 -and $probeHosts.Count -le 1)
    $v.foreign = $foreign
    $v.gits = $gits.Count
    $v.hostsUnder = $hosts.Count
    $v.hostsProbe = $probeHosts.Count
    $v.mine = $mine
    $v.treeSize = $tree.Count
    $v
}

function Write-Verdict($V, [string]$Label) {
    Write-Output ("{0}: git.exe seen under root {1}, process tree {2}" -f $Label, $V.gits, $V.treeSize)
    Write-Output ("{0}: visible console windows of the tree: {1}" -f $Label, @($V.mine).Count)
    foreach ($w in $V.mine) { Write-Output ("    t={0}ms {1} class={2} hwnd=0x{3:x} pid={4} title='{5}'" -f $w.T, $w.Ev, $w.Class, $w.Hwnd, $w.Pid, $w.Title) }
    Write-Output ("{0}: conhost.exe / OpenConsole.exe under the tree: {1} ({2} of the first git)" -f $Label, $V.hostsUnder, $V.hostsProbe)
    Write-Output ("{0}: console windows that are not the tree's: {1}" -f $Label, @($V.foreign).Count)
    foreach ($w in $V.foreign) { Write-Output ("    t={0}ms {1} class={2} hwnd=0x{3:x} pid={4} title='{5}'" -f $w.T, $w.Ev, $w.Class, $w.Hwnd, $w.Pid, $w.Title) }
}

function Get-ConhostCount { @(Get-Process -Name conhost, OpenConsole -ErrorAction SilentlyContinue).Count }

function Write-WatcherCost {
    $avg = if ([SpotWatch]::Loops) { [SpotWatch]::CycleTotalMs / [SpotWatch]::Loops } else { 0 }
    Write-Output ("watcher: snapshot every {0} ms, {1} snapshots, {2:N2} ms each on average (max {3:N1} ms), {4} window events seen ({5} gone before looked at, {6} of those at SHOW), {7} process records" -f $Interval, [SpotWatch]::Loops, $avg, [SpotWatch]::CycleMaxMs, [SpotWatch]::WinEvents, [SpotWatch]::Gone, [SpotWatch]::GoneShown, [SpotWatch]::Procs.Count)
}

if ($SelfTest) {
    $git = (Get-Command git.exe).Source
    # Git's own git.exe, whatever this shell's PATH finds: the detached child of a wrapper gets a console, so the case that must pass 46.11a skips the wrapper.
    $ownGit = Join-Path (((& $git --exec-path) | Select-Object -First 1) -replace '/', '\') 'git.exe'
    if (-not (Test-Path -LiteralPath $ownGit)) { Write-Output "check-spot-console: $ownGit does not exist"; exit 2 }
    $cmd = Join-Path $env:SystemRoot 'System32\cmd.exe'
    $NEW_CONSOLE = 0x10; $NO_WINDOW = 0x08000000; $DETACHED = 0x8
    # The helper: cmd running git and then staying a moment (so a console, if there is one, is up long enough to be seen).
    $viaCmd = "`"$cmd`" /D /C `"`"$git`" --version >nul & ping -n 2 127.0.0.1 >nul`""
    # A detached cmd has no console, so each git it starts opens its own. The alias keeps git alive for a whole snapshot cycle.
    $slowGit = "`"$ownGit`" -c `"alias.z=!ping -n 2 127.0.0.1`" z"
    $cases = @(
        @{ Name = 'visible console'; Cmd = $viaCmd; Flags = $NEW_CONSOLE; Want = @{ measured = $true; p4611 = $false } },
        @{ Name = 'CREATE_NO_WINDOW'; Cmd = $viaCmd; Flags = $NO_WINDOW; Want = @{ measured = $true; p4611 = $true; p4611a = $false } },
        @{ Name = 'DETACHED_PROCESS'; Cmd = "`"$ownGit`" -C `"$repo`" grep -c -e zzqqxx"; Flags = $DETACHED; Want = @{ measured = $true; p4611 = $true; p4611a = $true } },
        @{ Name = 'one console, the first git'; Cmd = "`"$cmd`" /D /C `"$slowGit`""; Flags = $DETACHED; Want = @{ measured = $true; p4611a = $true; hostsProbe = 1 } },
        @{ Name = 'a console per git'; Cmd = "`"$cmd`" /D /C `"$slowGit & $slowGit`""; Flags = $DETACHED; Want = @{ measured = $true; p4611a = $false; hostsProbe = 1 } },
        @{ Name = 'no process'; Cmd = $null; Flags = 0; Want = @{ measured = $false } }
    )
    $allOk = $true
    foreach ($c in $cases) {
        Write-Output "--- self-test: $($c.Name)"
        [SpotWatch]::Start($Interval)
        $root = 0
        if ($c.Cmd) {
            $root = [Spawn]::Start($c.Cmd, [uint32]$c.Flags, $env:SystemRoot)
            if (-not [Spawn]::Wait(15000)) { [Spawn]::Kill(); Write-Output '    (the helper had to be stopped)' }
        }
        [SpotWatch]::Stop()
        $v = Get-Verdict $root ([SpotWatch]::Procs.ToArray()) ([SpotWatch]::Wins.ToArray())
        Write-Verdict $v $c.Name
        Write-WatcherCost
        $ok = $true
        foreach ($k in $c.Want.Keys) { if ($v[$k] -ne $c.Want[$k]) { $ok = $false; Write-Output ("    expected {0} = {1}, got {2}" -f $k, $c.Want[$k], $v[$k]) } }
        if ($v.foreign.Count) { Write-Output '    note: another program put a console window up meanwhile (not counted)' }
        Write-Output ("self-test {0}: {1}" -f $c.Name, $(if ($ok) { 'as expected' } else { 'NOT as expected' }))
        if (-not $ok) { $allOk = $false }
    }
    if ($allOk) { Write-Output 'check-spot-console: self-test ok'; exit 0 }
    Write-Output 'check-spot-console: self-test FAILED'; exit 1
}

if (-not $Kura) { $Kura = Join-Path $repo 'target\release\kura.exe' }
if (-not (Test-Path -LiteralPath $Kura)) { Write-Output "check-spot-console: $Kura does not exist (cargo build --release, or pass -Kura)"; exit 2 }

# One run folder per run; -Out is not emptied.
$n = 1
while (Test-Path -LiteralPath (Join-Path $Out ('run-{0:D2}' -f $n))) { $n++ }
$run = Join-Path $Out ('run-{0:D2}' -f $n)
New-Item -ItemType Directory -Force -Path $run | Out-Null

# The repository: six files with two commits each, one modified afterwards, one untracked.
$tree = Join-Path $run 'repo'
New-Item -ItemType Directory -Force -Path $tree | Out-Null
$g = { param([string[]]$a) & git.exe -C $tree -c core.autocrlf=false @a 2>&1 | Out-Null; if ($LASTEXITCODE) { throw "git $($a -join ' ') failed" } }
try {
    & $g @('init', '-q', '-b', 'main')
    & $g @('config', 'user.email', 'check@example.invalid')
    & $g @('config', 'user.name', 'check')
    foreach ($i in 1..6) { [IO.File]::WriteAllText((Join-Path $tree "a$i.txt"), "first $i`n") }
    & $g @('add', '.'); & $g @('commit', '-q', '-m', 'first')
    foreach ($i in 1..6) { [IO.File]::AppendAllText((Join-Path $tree "a$i.txt"), "second $i`n") }
    & $g @('commit', '-q', '-a', '-m', 'second')
    [IO.File]::AppendAllText((Join-Path $tree 'a3.txt'), "modified`n")
    [IO.File]::WriteAllText((Join-Path $tree 'z-new.txt'), "untracked`n")
}
catch { Write-Output "check-spot-console: could not build the test repository: $_"; exit 2 }

# Isolation: each variable of lanes.conf's isolate= points at an empty folder of its own.
$vars = @()
foreach ($l in Get-Content -LiteralPath (Join-Path $PSScriptRoot 'lanes.conf')) { if ($l -match '^isolate=(.*)$') { $vars = $Matches[1] -split '\s+' | Where-Object { $_ } } }
if (-not $vars) { Write-Output 'check-spot-console: scripts\lanes.conf has no isolate='; exit 2 }
$saved = @{}
foreach ($name in ($vars + 'KURA_KEYS_DONE' + 'Path')) { $saved[$name] = [Environment]::GetEnvironmentVariable($name) }
$done = Join-Path $run 'keys.done'

# Keys: on each of a1..a6 open spot (rows: Name, Path, Kind, Mime, Size, Created, Modified, Accessed, Attributes, Lines, then the Git section: Last change,
# Subject, Author, Commits, then the text rows), go down ten rows and read, down three more and read, close, next file. Every key waits for the app to
# settle (no fixed sleep); the Git section arrives from the spot worker, so a read before it would show another row.
$down = '<A-j>' * 10
$keys = ''
foreach ($i in 1..6) { $keys += "<Tab>${down}<State:top$i><A-j><A-j><A-j><State:end$i><Tab>j" }
$keys += '<Quit>'

$p = $null; $timedOut = $false; $watching = $false; $setupError = $null
try {
    foreach ($name in $vars) {
        $d = Join-Path $run ('isolate\' + $name)
        New-Item -ItemType Directory -Force -Path $d | Out-Null
        [Environment]::SetEnvironmentVariable($name, $d)
    }
    $env:KURA_KEYS_DONE = $done
    # The PATH of a normal launch (see the header): which git.exe kura finds decides 46.11a.
    $env:Path = [Environment]::GetEnvironmentVariable('Path', 'Machine') + ';' + [Environment]::GetEnvironmentVariable('Path', 'User')
    $gitOnPath = (Get-Command git.exe -ErrorAction Stop | Select-Object -First 1).Source
    $before = Get-ConhostCount
    [SpotWatch]::Start($Interval)
    $watching = $true
    $p = Start-Process -FilePath $Kura -ArgumentList @('"' + $tree + '"', '--keys', ('"' + $keys + '"')) -PassThru
    $timedOut = -not $p.WaitForExit($Timeout * 1000)
    if ($timedOut) { Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue }  # only the process started here
}
catch { $setupError = "$_" }
finally {
    if ($watching) { [SpotWatch]::Stop() }
    foreach ($name in $saved.Keys) { [Environment]::SetEnvironmentVariable($name, $saved[$name]) }
}
if ($setupError) { Write-Output "check-spot-console: the run could not start: $setupError"; exit 2 }
$after = Get-ConhostCount

$procs = [SpotWatch]::Procs.ToArray(); $wins = [SpotWatch]::Wins.ToArray()
$v = Get-Verdict $p.Id $procs $wins
$procs | ForEach-Object { [pscustomobject]@{ Pid = $_.Pid; Ppid = $_.Ppid; Name = $_.Name; SeenMs = $_.Seen; LastMs = $_.Last } } | Export-Csv -LiteralPath (Join-Path $run 'processes.csv') -NoTypeInformation
$wins | ForEach-Object { [pscustomobject]@{ TMs = $_.T; Ev = $_.Ev; Class = $_.Class; Visible = $_.Visible; Pid = $_.Pid; Hwnd = ('0x{0:x}' -f $_.Hwnd); Title = $_.Title } } | Export-Csv -LiteralPath (Join-Path $run 'windows.csv') -NoTypeInformation

Write-Output "run folder: $run"
Write-Output "git.exe on the PATH kura was given: $gitOnPath"
Write-WatcherCost
Write-Output ("kura pid {0}, exit {1}" -f $p.Id, $(if ($timedOut) { 'timeout' } else { $p.ExitCode }))
Write-Verdict $v 'spot'
Write-Output ("conhost.exe + OpenConsole.exe on the machine: before {0}, after {1} (information only)" -f $before, $after)

# The Git section, from the state files.
$gitOk = $true; $gitLines = @()
foreach ($i in 1..6) {
    $row = @{}
    foreach ($w in 'end', 'top') {
        $f = Join-Path $run "$w$i.txt"
        $row[$w] = if (Test-Path -LiteralPath $f) { ((Select-String -LiteralPath $f -Pattern '^spot row: (.*)$' | Select-Object -First 1).Matches.Groups[1].Value) } else { '(no state)' }
        $pos = if (Test-Path -LiteralPath $f) { (Select-String -LiteralPath $f -Pattern '^spot: (.*)$' | Select-Object -First 1).Matches.Groups[1].Value } else { '' }
        $row["${w}pos"] = $pos
    }
    $ok = ($row.top -eq 'Last change' -and $row.end -eq 'Commits')
    if (-not $ok) { $gitOk = $false }
    $gitLines += ("    a{0}.txt: row 11 '{1}' ({2}), row 14 '{3}' ({4}) {5}" -f $i, $row.top, $row.toppos, $row.end, $row.endpos, $(if ($ok) { 'ok' } else { 'WRONG' }))
}
$last = if (Test-Path -LiteralPath $done) { Get-Content -LiteralPath $done | Select-Object -Last 1 } else { '(no done file)' }
$keysOk = (-not $timedOut) -and ($last -eq 'keys: done')

$bad = 0
function Say([bool]$Ok, [string]$Text) { if (-not $Ok) { $script:bad++ }; Write-Output ("{0,-5} {1}" -f $(if ($Ok) { 'ok' } else { 'FAIL' }), $Text) }
Say $keysOk $(if ($timedOut) { "keys: timed out after $Timeout s" } else { $last })
Say $v.measured ("measured: {0} git.exe seen under kura" -f $v.gits)
Say $v.p4611 ("46.11: {0} visible console windows of the tree" -f @($v.mine).Count)
Say $v.p4611a ("46.11a: {0} conhost.exe / OpenConsole.exe under the tree, {1} of them the first git's (one is allowed)" -f $v.hostsUnder, $v.hostsProbe)
Say $gitOk 'git: the Git section is there for all six files'
$gitLines | ForEach-Object { Write-Output $_ }
Say ([SpotWatch]::WinEvents -ge 1) ('events: the window hook saw {0} window events' -f [SpotWatch]::WinEvents)
Write-Output 'not covered: status marks and the branch name (no --keys reading shows them), a process shorter than one snapshot cycle, a window gone before its class was read'
if ($bad) { Write-Output 'check-spot-console: FAILED'; exit 1 }
if (@($v.foreign).Count) { Write-Output 'check-spot-console: inconclusive (a console window that is not the tree''s appeared meanwhile; run it again)'; exit 3 }
Write-Output 'check-spot-console: ok'
exit 0
