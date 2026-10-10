# Start the Windows test session by itself when there is new work for it.
#
# The session on the Windows machine (.claude/windows-role.md) is the only one
# that can run kura, and until now a person had to start it by hand for every
# section. This looks once, and starts one unattended run if:
#
#   - origin/main has changed TESTING.md, TESTING-CHECKS.md or
#     .claude/windows-role.md since the last run it started -- a merged run
#     changes the last two, so merging one pull request is what starts the next;
#   - no pull request from this lane (test/win-* or test/arm-*) is still open
#     (that section is not done yet, and a second run would take the same one:
#     it happened on 2026-09-28);
#   - the screen is not locked (SendInput does nothing on a locked desktop, and
#     the run would record every key as having done nothing);
#   - the working copy is clean. A dirty one is a run that was cut off: since
#     v0.101.0 its changes are committed to a branch of their own,
#     rescue/<lane>-<time>, and pushed, and the worktree goes back to
#     origin/main (Save-DirtyWorktree), so the lane carries on and nothing is
#     lost. Only when that cannot be done does the lane stop as it did before:
#     every firing says so in the log and in dirty<suffix>.txt in the state
#     folder, and a Windows notification says so once a day (see Note-Dirty).
#
# It is meant to be run by Task Scheduler once an hour at :20, as you, "only
# when the user is logged on" -- the run drives a real window. The merge-lanes
# workflow merges and shares within minutes; a firing while the lane's pull
# request is still open does nothing, so every 15 minutes only added firings that
# passed (the owner's word, 2026-10-05):
#
#   $w = 'C:\dev\filer-wintest'
#   $a = New-ScheduledTaskAction -Execute (Get-Command pwsh).Source -Argument "-NoProfile -WindowStyle Hidden -File $w\scripts\lane-boot.ps1"
#   $t = New-ScheduledTaskTrigger -Once -At (Get-Date -Minute 20 -Second 0) -RepetitionInterval (New-TimeSpan -Hours 1)
#   $s = New-ScheduledTaskSettingsSet -MultipleInstances IgnoreNew -ExecutionTimeLimit (New-TimeSpan -Hours 4)
#   Register-ScheduledTask -TaskName filer-auto-wintest -Action $a -Trigger $t -Settings $s
#
#   Unregister-ScheduledTask -TaskName filer-auto-wintest    # to stop it
#
# pwsh is given by its full path: on 2026-10-09 a task registered with a bare
# `pwsh` ended every firing with 0x80070002 (2147942402, file not found)
# before the script ran, so nothing reached the log.
# If that path is under C:\Program Files\WindowsApps (the Store's PowerShell),
# it names the version and stops working at the next update: give the task
# $env:LOCALAPPDATA\Microsoft\WindowsApps\pwsh.exe (the Store's own alias,
# which follows updates) or install the MSI build (C:\Program Files\PowerShell\7).
#
# The task runs scripts/lane-boot.ps1, which fetches main and runs main's copy
# of this script from the state folder (boot<suffix>), whatever state the
# worktree is in, so a fix on main reaches the next firing. Before v0.101.0
# the task's -Command moved the worktree to origin/main only when it was clean
# and ran the worktree's copy, so a dirty worktree kept an old copy (and
# before v0.80.13 a copy that did not parse could not even fetch its fix:
# neither lane opened a pull request from 2026-10-06 to 2026-10-09). CI parses
# every script here (scripts/check-ps1.ps1), and lane-boot.ps1 runs the copy it
# took last time when main's does not parse. For the ARM64 machine, $w is
# C:\dev\filer-armtest, the task filer-auto-wintest-arm, and ` -Lane arm` goes
# after the .ps1. To move an existing task over, run the two lines above with
# the right $w (and -Lane), then:
#
#   Set-ScheduledTask -TaskName filer-auto-wintest -Action $a
#
# The task names the worktree's lane-boot.ps1, never the checkout you work in,
# which only moves when someone pulls: until v0.73.24 the ARM64 laptop ran
# v0.51.1 for days that way (#201). The very first time, before the worktree
# exists, run scripts\lane-boot.ps1 once by hand from your checkout to make it
# (without -Force it makes the worktree and stops if there is nothing to run).
#
# By hand:
#
#   pwsh -File scripts\lane-boot.ps1             # main's copy: look once, run if there is work
#   pwsh -File scripts\auto-wintest.ps1          # this copy, the same
#   pwsh -File scripts\auto-wintest.ps1 -Force   # run even if nothing changed
#   ... -LogDir R:\Temp                           # the log on the RAM disk
#   ... -Lane arm                                 # the ARM64 machine's lane
#   ... -TargetOnDisk                             # keep the build output on C:
#   ... -Model claude-opus-5-5                    # another model for this run
#
# Lanes. The x64 machine runs lane `win` (the default), the ARM64 laptop lane
# `arm`. Each has its own queue in windows-role.md, its own branch prefix
# (test/win-, test/arm-), its own worktree and state, and waits only on its
# own pull requests -- so the two machines never take the same section and
# never hold each other up.
#
# Scratch space. Each run gets a folder of its own, run-<time>, under -Scratch,
# which defaults to R:\Temp when there is an R: drive (the RAM disk on the x64
# machine) and to %TEMP%\kura-scratch otherwise. TEMP and TMP point into it and
# the prompt says where it is; `cargo test` builds its test trees under TEMP, so
# they follow. Only the newest three run-* folders are kept: on the ARM64
# laptop, with no RAM disk to empty itself, ten runs' leftovers had piled up and
# an old run's script error dialog was still open in the middle of the screen
# (#103). Directly under -Scratch, kura-test-* and kura-archive-* folders older
# than three days are removed too (#259: 11,187 of them, 1.6 GB, had piled up on
# the ARM64 laptop from before the run-* folders). Nothing else is touched.
#
# The screen saver is held off for the length of a run (-KeepScreenSaver
# leaves it alone). A screen saver owns the input desktop, and SendInput then
# goes nowhere without an error: on the ARM64 laptop three runs in a row lost
# every mouse row that way (#93, #100). Three layers, because the laptop's saver
# is not Windows' own -- ASUS OLED Care starts `OLED Care Screensaver.scr` by
# itself, with Windows' screen saver set to (None):
#
#   - SetThreadExecutionState(ES_DISPLAY_REQUIRED) for as long as the run lasts,
#     which is what keeps a video player's screen on;
#   - Windows' own saver switched off in memory only (SPI_SETSCREENSAVEACTIVE
#     with no SPIF_UPDATEINIFILE), so nothing is written to the profile, and
#     the old value put back afterwards;
#   - a watcher that stops any running `*.scr` every 5 seconds, for a saver
#     that heeds neither of the above.
#
# Everything is undone in `finally`. A run killed outright never reaches it, so
# the old value is also written to screensaver.json first, and the next firing
# puts it back before doing anything else.
#
# Status on GitHub. Every firing writes the lane's issue, "Lane status: win"
# or "Lane status: arm" (label lane-status, opened by the first firing), again:
# when it fired, which copy of this script, what the firing came to, whether
# the worktree is dirty, the failed runs in a row and this firing's log lines
# (scripts/lane-status.ps1). A run also writes it when it starts, and a rescue
# of a dirty worktree adds a comment (a comment notifies; an edit does not). A
# cloud session reads it with
#   gh api 'repos/uchmk/kura/issues?labels=lane-status&state=all'
# A failure to write it is logged and changes nothing else. A lane whose task
# stops firing writes nothing at all, so the Merge lanes workflow watches the
# issue from outside: a **Last firing** more than three hours old (five while a
# run is going) gets the label lane-stalled and one comment
# (scripts/merge-lanes.py, watch_lanes). Close the issue of a lane that is
# retired.
#
# Log: %LOCALAPPDATA%\filer-wintest\auto-wintest.log, or in -LogDir. The log
# may go on the RAM disk (-LogDir R:\Temp): it is for reading what a run did,
# not evidence. The state file stays in %LOCALAPPDATA% whatever -LogDir says --
# on R: it would be gone after a reboot, and the next firing would start a run
# for a trigger that was already used.
#
# Build output. The worktree's `target` is made a junction to a folder on the
# RAM disk, R:\cargo-target\<worktree name> (or -TargetDir), so the role's
# paths (`target\release\kura.exe`) still work and the bytes live in memory.
# On 2026-10-03 C:\dev held 45 GB, and the two `target` folders were 33 GB of
# it. After a reboot the RAM disk is empty: the folder is made again and the
# first build takes a few minutes longer, nothing worse. With less than 8 GB
# free on R: the run builds on C: as before, and says so in the log.
# -TargetOnDisk turns all of this off. Incremental compilation is off for the
# run as well: an unattended run builds once, and its caches were 6 GB.
#
# Model. The run is given --model $Model, claude-sonnet-5-5 by default, so the
# lanes do not follow whatever model the machine's own `claude` was last set to.
# The lanes ran on Opus until v0.80.15; on 2026-10-09 the owner moved both
# lanes, and the merge routine, to Sonnet 5.5. To change it, add -Model to the
# task's arguments; the log names the model of every run.
# A `claude` too old for the model is updated with `claude update` and the run
# tried once more (v0.78.104); three failed runs in a row put a line of
# `!!!!!` in the log.
#
# The desktop. tsumugi's lane (uchmk/tsumugi, :50) drives the same desktop
# with SendInput, so both scripts hold Local\wintest-desktop while their run
# goes, and wait up to -DesktopWaitMin minutes (20) for the other. Still held
# after that: the trigger is left for the next firing, and the log says
# "desktop lock: still held". A wait is logged as "desktop lock: waited N min".
#
# The run works in its own worktree ($Work), not in the checkout you use, so
# it never meets your uncommitted changes and you can keep working while it
# runs. Needs `claude` and an authenticated `gh` on PATH.
#
# What the run may do is below in $Tools. It is broad on purpose -- a test run
# writes and runs its own PowerShell to drive the window -- so the guard is the
# role definition, plus the few commands it must never run, in $Denied.

param(
    [ValidateSet('win', 'arm')] [string]$Lane = 'win',
    [string]$Work,
    [string]$LogDir,
    [string]$Scratch,
    [switch]$Force,
    [switch]$KeepScreenSaver,
    [string]$TargetDir,
    [switch]$TargetOnDisk,
    [string]$Model = 'claude-sonnet-5-5',
    [int]$DesktopWaitMin = 20,
    # Given by lane-boot.ps1: the origin/main commit this copy was taken from,
    # and the checkout lane-boot.ps1 is in (the worktree is made from it).
    [string]$Booted,
    [string]$From
)

$ErrorActionPreference = 'Stop'

$repo = 'uchmk/kura'
$watched = @('TESTING.md', 'TESTING-CHECKS.md', '.claude/windows-role.md')
$Tools = 'Bash,PowerShell,Read,Edit,Write,Glob,Grep,TodoWrite'
$Denied = @(
    'Bash(git push origin main:*)', 'PowerShell(git push origin main:*)',
    'Bash(git push -f:*)', 'PowerShell(git push -f:*)',
    'Bash(git push --force:*)', 'PowerShell(git push --force:*)',
    'Bash(gh pr merge:*)', 'PowerShell(gh pr merge:*)',
    'Bash(cargo fmt:*)', 'PowerShell(cargo fmt:*)'
) -join ','

# The `win` lane keeps the names it had before lanes existed, so a machine
# already running it carries on from its state file.
$suffix = if ($Lane -eq 'win') { '' } else { "-$Lane" }
if (-not $Work) { $Work = if ($Lane -eq 'win') { 'C:\dev\filer-wintest' } else { "C:\dev\filer-$($Lane)test" } }
if (-not $Scratch) {
    $Scratch = if (Test-Path 'R:\') { 'R:\Temp' } else { Join-Path ([IO.Path]::GetTempPath()) 'kura-scratch' }
}
$queue = if ($Lane -eq 'win') { 'the queue in "Where the work is"' } else { 'the ARM64 queue in "The ARM64 lane"' }

$state = Join-Path $env:LOCALAPPDATA 'filer-wintest'
New-Item -ItemType Directory -Force -Path $state | Out-Null
if (-not $LogDir) { $LogDir = $state }
New-Item -ItemType Directory -Force -Path $LogDir | Out-Null
New-Item -ItemType Directory -Force -Path $Scratch | Out-Null
$log = Join-Path $LogDir "auto-wintest$suffix.log"
$last = Join-Path $state "last-trigger$suffix"

# Points $Work\target at the RAM disk (see the top). Returns where the build
# output goes, for the log.
function Set-BuildTarget {
    $link = Join-Path $Work 'target'
    if ($TargetOnDisk) { return $link }
    $dir = if ($TargetDir) { $TargetDir } elseif (Test-Path 'R:\') { Join-Path 'R:\cargo-target' (Split-Path -Leaf $Work) } else { $null }
    if (-not $dir) { return $link }
    $drive = Get-PSDrive -Name ($dir.Substring(0, 1)) -ErrorAction SilentlyContinue
    $item = Get-Item -LiteralPath $link -Force -ErrorAction SilentlyContinue
    $isLink = $item -and ($item.Attributes -band [IO.FileAttributes]::ReparsePoint)
    if (-not $isLink -and $drive -and $drive.Free -lt 8GB) {
        # Out-Host: Say also writes to the output, which here is the return value.
        Say ("Only {0:N1} GB free on {1}: building in {2} this time." -f ($drive.Free / 1GB), $drive.Root, $link) | Out-Host
        return $link
    }
    New-Item -ItemType Directory -Force -Path $dir | Out-Null
    if ($isLink -and ("$($item.Target)" -eq $dir)) { return $dir }
    if ($isLink) {
        [IO.Directory]::Delete($link)                    # the junction only, not what it points at
    } elseif ($item) {
        $gb = (Get-ChildItem -LiteralPath $link -Recurse -File -Force -ErrorAction SilentlyContinue | Measure-Object Length -Sum).Sum / 1GB
        Remove-Item -Recurse -Force -LiteralPath $link   # build output: the next build makes it again
        Say ("Removed {0} ({1:N1} GB) to build on {2} instead." -f $link, $gb, $dir) | Out-Host
    }
    New-Item -ItemType Junction -Path $link -Target $dir | Out-Null
    return $dir
}

# This firing's lines, for the lane's status issue (Publish-Status).
$said = [Collections.Generic.List[string]]::new()
function Say([string]$line) {
    $stamped = '[{0:yyyy-MM-dd HH:mm:ss}] {1}' -f (Get-Date), $line
    $stamped
    Add-Content -Path $log -Value $stamped
    [void]$script:said.Add($stamped)
}

# A notification in the corner of the screen (a balloon, which Windows 10 and
# 11 show as a toast). NotifyIcon rather than the WinRT toast API, which pwsh 7
# cannot load. Not yet run on Windows.
function Show-Notice([string]$title, [string]$text) {
    try {
        Add-Type -AssemblyName System.Windows.Forms, System.Drawing
        $icon = [Windows.Forms.NotifyIcon]::new()
        $icon.Icon = [Drawing.SystemIcons]::Warning
        $icon.Text = 'kura auto-wintest'
        $icon.Visible = $true
        if ($text.Length -gt 250) { $text = $text.Substring(0, 250) + '...' }
        $icon.ShowBalloonTip(30000, $title, $text, [Windows.Forms.ToolTipIcon]::Warning)
        Start-Sleep -Seconds 10   # the balloon goes with the icon
        $icon.Dispose()
    } catch {
        Say "Could not show a notification: $_"
    }
}

# A dirty worktree that Save-DirtyWorktree could not save stops every run
# until a person cleans it. On 2026-09-30 a cut-off run left TESTING-CHECKS.md
# half written, and for two days the lane made no pull request and said
# nothing anywhere: Task Scheduler showed only LastTaskResult 1, and the log
# was on the RAM disk. So the reason goes into the state folder (whatever
# -LogDir says), with the time it was first seen, and a notification says so
# when first seen and then once a day.
$dirtyFile = Join-Path $state "dirty$suffix.txt"
function Note-Dirty([string[]]$changes) {
    $now = Get-Date
    $since = $now
    $told = [datetime]::MinValue
    if (Test-Path $dirtyFile) {
        foreach ($line in Get-Content $dirtyFile) {
            if ($line -match '^since: (.+)$') { $since = [datetime]::Parse($Matches[1]) }
            if ($line -match '^told: (.+)$') { $told = [datetime]::Parse($Matches[1]) }
        }
    }
    $hours = [int]($now - $since).TotalHours
    $what = "$Work has uncommitted changes (first seen $('{0:yyyy-MM-dd HH:mm}' -f $since), $hours h ago). No run starts until a person looks at them and cleans the worktree."
    $tell = ($now - $told).TotalHours -ge 24
    if ($tell) {
        Say "!!!!! [$Lane] $what !!!!!"
        Show-Notice "kura auto-wintest ($Lane) is stopped" "$what $($changes.Count) changed path(s), e.g. $($changes[0].Trim())"
        $told = $now
    }
    $body = @(
        "since: $('{0:o}' -f $since)"
        "told: $('{0:o}' -f $told)"
        "checked: $('{0:o}' -f $now)"
        $what
        ''
        'git status --porcelain:'
    ) + @($changes | Select-Object -First 40)
    Set-Content -Path $dirtyFile -Value $body
}

# A dirty worktree is a run that was cut off. Until v0.101.0 it stopped every
# run until a person cleaned it, and lanes sat still for days that way
# (2026-09-30, 2026-10-10). Now its changes are committed to a branch of their
# own, rescue/<lane>-<time>, which is pushed (kept locally when the push
# fails), and the worktree goes back to origin/main: nothing is lost and the
# lane carries on. Only in a lane's own worktree -- on a detached HEAD, a
# test/<lane>-* branch or an earlier rescue/<lane>-* branch, which is all a
# lane's worktree is ever on -- and never in the middle of a rebase. Returns
# the branch, or $null when the worktree is left as it is (Note-Dirty then
# stops the run). Say goes to the host: its output would be the return value.
$rescued = $null
function Save-DirtyWorktree([string[]]$changes) {
    $head = (git -C $Work rev-parse --abbrev-ref HEAD 2>$null | Out-String).Trim()
    if ($head -ne 'HEAD' -and $head -notlike "test/$Lane-*" -and $head -notlike "rescue/$Lane-*") {
        Say "$Work is on $head, not a lane's branch: its changes are left alone." | Out-Host
        return $null
    }
    foreach ($dir in 'rebase-merge', 'rebase-apply') {
        if (Test-Path (git -C $Work rev-parse --path-format=absolute --git-path $dir | Out-String).Trim()) {
            Say "$Work is in the middle of a rebase: its changes are left alone." | Out-Host
            return $null
        }
    }
    $on = if ($head -eq 'HEAD') { 'a detached HEAD' } else { $head }
    $branch = 'rescue/{0}-{1:yyyyMMdd-HHmm}' -f $Lane, (Get-Date)
    $was = (git -C $Work rev-parse --short HEAD | Out-String).Trim()
    try {
        git -C $Work checkout -q -B $branch
        if ($LASTEXITCODE -ne 0) { throw "git checkout -B $branch failed" }
        git -C $Work add -A
        if ($LASTEXITCODE -ne 0) { throw 'git add -A failed' }
        $what = "auto-wintest.ps1 found $($changes.Count) uncommitted path(s) in the $Lane lane's worktree on $on ($was), left by a run that was cut off, and saved them here before putting the worktree back on origin/main."
        git -C $Work commit -q --no-verify -m "Rescue the $Lane lane's uncommitted changes" -m $what
        if ($LASTEXITCODE -ne 0) { throw 'git commit failed' }
        git -C $Work push -q origin "refs/heads/${branch}:refs/heads/$branch" 2>$null
        $pushed = $LASTEXITCODE -eq 0
        git -C $Work checkout -q --detach origin/main
        if ($LASTEXITCODE -ne 0) { throw 'git checkout origin/main failed' }
    } catch {
        Say "Could not save the dirty worktree to ${branch}: $_" | Out-Host
        return $null
    }
    $where = if ($pushed) { "pushed to origin as $branch" } else { "committed to the local branch $branch (the push failed: it is only on this machine)" }
    Say "!!!!! [$Lane] $Work had $($changes.Count) uncommitted path(s) on $on ($was): $where, and the worktree is back on origin/main. !!!!!" | Out-Host
    $script:rescued = @(
        "**Rescued a dirty worktree.** It had $($changes.Count) uncommitted path(s) on $on (``$was``), left by a run that was cut off. They are $where, and the worktree is back on origin/main, so the lane carries on."
        ''
        'Look at the branch: merge what is worth keeping (a run''s ticks need their evidence, as in any lane pull request) and delete it.'
        ''
        '```text'
    ) + @($changes | Select-Object -First 40) + @('```')
    if (Test-Path $dirtyFile) { Remove-Item -LiteralPath $dirtyFile }
    $branch
}

. (Join-Path $PSScriptRoot 'lane-marks.ps1')
. (Join-Path $PSScriptRoot 'lane-status.ps1')

# This firing, written into the lane's status issue on GitHub (lane-status.ps1)
# at the start of a run and at the end of every firing, so a cloud session can
# tell why a lane is not making pull requests. $outcome says what the firing
# came to; when it is not set, the last line it logged does. Never throws.
$outcome = $null
$failFile = Join-Path $state "failures$suffix"
function Publish-Status {
    try {
        $what = if ($script:outcome) { $script:outcome } elseif ($said.Count) { $said[-1] -replace '^\[[^\]]*\] ', '' } else { 'Nothing to do.' }
        $now = Get-Date
        $history = @(Add-LaneHistory (Join-Path $state "status-history$suffix.txt") ('{0:yyyy-MM-dd HH:mm} {1}' -f $now, $what))
        $toml = Get-Content -Raw -LiteralPath (Join-Path (Split-Path -Parent $PSScriptRoot) 'Cargo.toml') -ErrorAction SilentlyContinue
        $version = if ($toml -match '(?m)^version = "([^"]+)"') { "v$($Matches[1])" } else { '?' }
        $copy = if ($Booted) { "$version ($($Booted.Substring(0, [Math]::Min(7, $Booted.Length))), main's copy by lane-boot.ps1)" }
                else { "$version ($((git -C $PSScriptRoot rev-parse --short HEAD 2>$null) -join ''), $PSCommandPath)" }
        $dirty = if (Test-Path $Work) { @(git -C $Work status --porcelain 2>$null) } else { @() }
        $fails = if (Test-Path $failFile) { [int](Get-Content -Raw -LiteralPath $failFile) } else { 0 }
        $task = if ($Lane -eq 'win') { 'filer-auto-wintest' } else { "filer-auto-wintest-$Lane" }
        $body = Format-LaneStatus -Lane $Lane -When $now -Script $copy -Outcome $what -Task $task `
            -Said @($said) -Dirty $dirty -Failures $fails -History @($history | Select-Object -Skip 1)
        $err = Publish-LaneStatus -Repo $repo -Lane $Lane -Body $body -StateDir $state
        if ($err) { Say "Could not write the lane's status issue: $err" }
        if ($script:rescued -and -not $err) {
            $err = Add-LaneComment -Repo $repo -Lane $Lane -Body ($script:rescued -join "`n") -StateDir $state
            if ($err) { Say "Could not comment on the lane's status issue: $err" }
            $script:rescued = $null
        }
    } catch {
        Say "Could not write the lane's status issue: $_"
    }
}

# A run takes an hour and main moves under it: a row reworded, another lane's
# ticks, a re-test list changed. The run's marks then sit beside lines main
# changed, and its pull request opens in conflict and waits for the Merge
# lanes workflow or a person. When only the checklists conflict, merge main
# into the run's branch with main's checklists and the run's marks made again
# on them (Set-LaneMarks), and push, so the pull request is mergeable from the
# start. A row main reworded or took out keeps no mark, and the pull request
# says so. Anything else that conflicts is left as it is. Never throws.
function Sync-LaneBranch([string]$Branch) {
    $lists = @('TESTING-CHECKS.md', 'TESTING-KEYS.md')
    Push-Location $Work
    try {
        if ((git rev-parse --abbrev-ref HEAD).Trim() -ne $Branch) { Say "Sync: $Work is not on $Branch; left as it is."; return }
        git fetch -q origin main $Branch
        if ($LASTEXITCODE -ne 0) { Say 'Sync: git fetch failed; left as it is.'; return }
        # Only what the run pushed: an unpushed commit stays the run's.
        if ((git rev-parse HEAD).Trim() -ne (git rev-parse "origin/$Branch").Trim()) { Say "Sync: $Branch is not what was pushed; left as it is."; return }
        git merge-base --is-ancestor origin/main HEAD
        if ($LASTEXITCODE -eq 0) { return }
        $base = (git merge-base origin/main HEAD).Trim()
        git merge -q --no-ff --no-commit origin/main *> $null
        $conflicted = @(git diff --name-only --diff-filter=U)
        if (-not $conflicted) { git merge --abort; return }   # GitHub merges it as it is
        $others = @($conflicted | Where-Object { $_ -notin $lists })
        if ($others) { git merge --abort; Say "Sync: $Branch conflicts with main in $($others -join ', '); left for a person."; return }
        $dropped = @()
        foreach ($path in $conflicted) {
            $marks = Get-LaneMarks -Base ((git show "${base}:$path") -join "`n") -Branch ((git show "HEAD:$path") -join "`n")
            git checkout -q --theirs -- $path
            $file = Join-Path $Work $path
            $r = Set-LaneMarks -Main ([IO.File]::ReadAllText($file)) -Marks $marks
            [IO.File]::WriteAllText($file, $r.Text, $utf8)
            git add -- $path
            $dropped += $r.Dropped
        }
        git commit -q --no-edit
        if ($LASTEXITCODE -ne 0) { throw 'git commit failed' }
        git push -q origin "HEAD:refs/heads/$Branch"
        if ($LASTEXITCODE -ne 0) { Say "Sync: git push failed; $Branch stays in conflict for the Merge lanes workflow."; return }
        Say "Sync: merged main into $Branch with the run's marks made again ($($conflicted -join ', '))."
        if ($dropped) {
            $body = "Merged main into this branch: main changed these rows while the run was going, so their marks were not kept (what the run checked is not what the rows say now): $($dropped -join ', ')."
            gh pr comment $Branch --repo $repo --body $body *> $null
            Say "Sync: marks dropped for $($dropped -join ', ')."
        }
    } catch {
        Say "Sync: $_; left as it is."
    } finally {
        git rev-parse -q --verify MERGE_HEAD *> $null
        if ($LASTEXITCODE -eq 0) { git merge --abort }
        Pop-Location
    }
}

# The screen saver, held off while a run drives the window (see the top).
$saverFile = Join-Path $state "screensaver$suffix.json"
Add-Type -Namespace KuraWintest -Name Power -MemberDefinition @'
[DllImport("kernel32.dll")]
public static extern uint SetThreadExecutionState(uint flags);
[DllImport("user32.dll", SetLastError = true)]
public static extern bool SystemParametersInfo(uint action, uint param, ref bool value, uint winIni);
[DllImport("user32.dll", SetLastError = true)]
public static extern bool SystemParametersInfo(uint action, uint param, System.IntPtr value, uint winIni);
[DllImport("user32.dll", SetLastError = true)]
public static extern System.IntPtr OpenInputDesktop(uint flags, bool inherit, uint access);
[DllImport("user32.dll", SetLastError = true)]
public static extern bool CloseDesktop(System.IntPtr desktop);
[DllImport("user32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
public static extern bool GetUserObjectInformation(System.IntPtr obj, int index, System.Text.StringBuilder info, int length, out int needed);
'@
$ES_CONTINUOUS = [uint32]'0x80000000'
$ES_SYSTEM_REQUIRED = [uint32]1
$ES_DISPLAY_REQUIRED = [uint32]2
$SPI_GETSCREENSAVEACTIVE = [uint32]16
$SPI_SETSCREENSAVEACTIVE = [uint32]17

function Get-SaverActive {
    $on = $false
    [void][KuraWintest.Power]::SystemParametersInfo($SPI_GETSCREENSAVEACTIVE, 0, [ref]$on, 0)
    $on
}

# In memory only: winIni 0, so the profile keeps whatever the owner chose.
function Set-SaverActive([bool]$on) {
    $ok = [KuraWintest.Power]::SystemParametersInfo($SPI_SETSCREENSAVEACTIVE, [uint32][int]$on, [IntPtr]::Zero, 0)
    # Laptops answer 329 (#201): the saver cannot be switched, only `Stop-ScreenSavers` holds it off.
    if (-not $ok) { Say "SystemParametersInfo(SPI_SETSCREENSAVEACTIVE, $on) failed, GetLastError $([Runtime.InteropServices.Marshal]::GetLastWin32Error()); the screen saver setting is unchanged." }
}

# A run killed before its `finally` left the saver off; put it back first.
function Restore-LeftOverSaver {
    if (-not (Test-Path $saverFile)) { return }
    $was = (Get-Content -Raw $saverFile | ConvertFrom-Json).active
    Set-SaverActive $was
    Remove-Item $saverFile
    Say "Put the screen saver back (active = $was), left off by a run that was cut off."
}

# The desktop that receives input: `Default` when keys and clicks reach the
# windows on it, `Screen-saver` or `Winlogon` when they go nowhere. Asked
# rather than inferred from LogonUI, which a screen saver does not start (#88).
function Get-InputDesktop {
    $h = [KuraWintest.Power]::OpenInputDesktop(0, $false, 0x0001)   # DESKTOP_READOBJECTS
    if ($h -eq [IntPtr]::Zero) { return '(none: OpenInputDesktop failed)' }
    try {
        $name = [Text.StringBuilder]::new(256)
        $needed = 0
        [void][KuraWintest.Power]::GetUserObjectInformation($h, 2, $name, 512, [ref]$needed)   # UOI_NAME
        $name.ToString()
    } finally {
        [void][KuraWintest.Power]::CloseDesktop($h)
    }
}

function Stop-ScreenSavers {
    Get-Process | Where-Object { $_.Path -like '*.scr' } | ForEach-Object {
        Stop-Process -Id $_.Id -Force -ErrorAction SilentlyContinue
        $_.Path
    }
}

function Suspend-ScreenSaver {
    $was = Get-SaverActive
    @{ active = $was } | ConvertTo-Json | Set-Content -Path $saverFile
    Set-SaverActive $false
    [void][KuraWintest.Power]::SetThreadExecutionState($ES_CONTINUOUS -bor $ES_SYSTEM_REQUIRED -bor $ES_DISPLAY_REQUIRED)
    foreach ($p in Stop-ScreenSavers) { Say "Stopped a running screen saver: $p" }
    # The watcher. A thread job shares nothing with this script but what it is
    # given, so the function goes in as text.
    $body = ${function:Stop-ScreenSavers}.ToString()
    $script:saverWatch = Start-ThreadJob -ArgumentList $body -ScriptBlock {
        param($body)
        $stop = [scriptblock]::Create($body)
        while ($true) {
            foreach ($p in & $stop) { "{0:HH:mm:ss} stopped {1}" -f (Get-Date), $p }
            Start-Sleep -Seconds 5
        }
    }
    Say "Holding the screen saver off for the run (it was active = $was)."
}

function Resume-ScreenSaver {
    if ($script:saverWatch) {
        $stopped = Receive-Job $script:saverWatch -ErrorAction SilentlyContinue
        Remove-Job $script:saverWatch -Force
        $script:saverWatch = $null
        if ($stopped) { Say "During the run the watcher stopped a screen saver $(@($stopped).Count) time(s): $(@($stopped)[-1])" }
    }
    [void][KuraWintest.Power]::SetThreadExecutionState($ES_CONTINUOUS)
    if (Test-Path $saverFile) {
        $was = (Get-Content -Raw $saverFile | ConvertFrom-Json).active
        Set-SaverActive $was
        Remove-Item $saverFile
        Say "Gave the screen saver back (active = $was)."
    }
}

# Task Scheduler's IgnoreNew already keeps its own firings apart; this also
# covers one started by hand while a scheduled one is running.
$mutex = [Threading.Mutex]::new($false, "Local\filer-auto-wintest$suffix")
if (-not $mutex.WaitOne(0)) { Say 'A run is already going. Nothing to do.'; exit 0 }
$desktop = $null

try {
    Restore-LeftOverSaver

    if (Get-Process LogonUI -ErrorAction SilentlyContinue) {
        Say 'The screen is locked. Trying again next time.'
        exit 0
    }

    if (-not (Test-Path $Work)) {
        # The worktree hangs off the checkout lane-boot.ps1 or this script is in.
        $main = if ($From) { $From } else { Split-Path -Parent $PSScriptRoot }
        git -C $main fetch -q origin main
        git -C $main worktree add -q --detach $Work origin/main
        Say "Made the worktree $Work."
    }

    git -C $Work fetch -q origin main
    if ($LASTEXITCODE -ne 0) { Say 'git fetch failed. Trying again next time.'; exit 0 }
    # Kept on origin/main at every firing, not only when a run starts, so the
    # worktree's lane-boot.ps1 follows main too. A dirty worktree is saved to
    # a rescue/ branch first (Save-DirtyWorktree), before the trigger check
    # that quietly ends most firings; when it cannot be, it stops the run
    # further down.
    $changes = @(git -C $Work status --porcelain)
    if ($changes.Count -and (Save-DirtyWorktree $changes)) { $changes = @() }
    if ($changes.Count -eq 0) {
        git -C $Work checkout -q --detach origin/main
        if (Test-Path $dirtyFile) {
            Say "$Work is clean again."
            Remove-Item -LiteralPath $dirtyFile
        }
    } else {
        Note-Dirty $changes
    }

    $trigger = (git -C $Work log -1 --format=%H origin/main -- $watched).Trim()
    $seen = if (Test-Path $last) { (Get-Content -Raw $last).Trim() } else { '' }
    if (-not $Force -and $trigger -eq $seen) {   # quiet: this is most runs
        $outcome = "Nothing new on main since the last run (trigger $($trigger.Substring(0, 7)))."
        if ($changes.Count) { $outcome += " The worktree is dirty and could not be saved, so new work would not start either." }
        if ($rescued) { $outcome = "Saved a dirty worktree to a rescue/ branch (see the comment). $outcome" }
        exit 0
    }

    # A merge that only records a lane's result (its "the test suite" row and
    # its "When every row above is empty" row in windows-role.md) is not new
    # work: starting on it made an empty queue wake itself every hour (#291,
    # #292). The trigger is used up without a run.
    $seenKnown = $false
    if ($seen) { git -C $Work cat-file -e "$seen^{commit}" 2>$null; $seenKnown = ($LASTEXITCODE -eq 0) }
    if (-not $Force -and $seenKnown) {
        $files = @(git -C $Work diff --name-only $seen $trigger -- $watched)
        if ($files.Count -eq 1 -and $files[0] -eq '.claude/windows-role.md') {
            $changed = @(git -C $Work diff -U0 $seen $trigger -- '.claude/windows-role.md' |
                Where-Object { $_ -match '^[+-]' -and $_ -notmatch '^(\+\+\+|---) ' })
            $other = @($changed | Where-Object { $_ -notmatch '^[+-]\| \*\*(the test suite|When every row above is empty)\*\* \|' })
            if ($changed.Count -gt 0 -and $other.Count -eq 0) {
                Set-Content -NoNewline -Path $last -Value $trigger
                $outcome = "Main changed only a lane's record in windows-role.md (trigger $($trigger.Substring(0, 7))): nothing to run."
                exit 0
            }
        }
    }

    $open = gh pr list --repo $repo --state open --json headRefName --jq '.[].headRefName' |
        Where-Object { $_ -like "test/$Lane-*" }
    if ($LASTEXITCODE -ne 0) { Say 'gh pr list failed (is gh logged in?). Trying again next time.'; exit 0 }
    if ($open) {
        Say "Waiting: $($open -join ', ') is still open."
        exit 0
    }

    # Since v0.86.6 the merge-lanes workflow merges this lane's pull request
    # as soon as its checks are green, and since v0.86.11 pushes the share
    # (CHANGELOG.md, the report's items into TODO.md) right after. Starting between
    # the two would take the same rows again, so wait while the lane's latest
    # merged pull request is not named in main's CHANGELOG.md yet.
    $merged = gh pr list --repo $repo --state merged --limit 30 --json number,headRefName,mergedAt
    if ($LASTEXITCODE -ne 0) { Say 'gh pr list failed (is gh logged in?). Trying again next time.'; exit 0 }
    $lastMerged = $merged | ConvertFrom-Json | Where-Object { $_.headRefName -like "test/$Lane-*" } |
        Sort-Object { [datetime]$_.mergedAt } -Descending | Select-Object -First 1
    if ($lastMerged -and [datetime]$lastMerged.mergedAt -gt (Get-Date).AddDays(-3)) {
        $changelog = (git -C $Work show origin/main:CHANGELOG.md) -join "`n"
        if ($changelog -notmatch "#$($lastMerged.number)(?!\d)") {
            Say "Waiting: #$($lastMerged.number) is merged, but the merger's share for it is not on main yet."
            exit 0
        }
    }

    if (git -C $Work status --porcelain) {
        Say "$Work has uncommitted changes, left by a run that was cut off, and they could not be saved to a rescue/ branch (the lines above say why). Look at them (listed in $dirtyFile), then clean it (git -C $Work stash -u, or git restore/clean) and run again."
        $outcome = "Stopped: $Work has uncommitted changes that could not be saved to a rescue/ branch. A person has to look at them and clean the worktree."
        exit 1
    }

    git -C $Work checkout -q --detach origin/main
    $head = (git -C $Work rev-parse --short HEAD).Trim()
    Say "[$Lane] Starting a run on $head (trigger $($trigger.Substring(0, 7)))."
    # Which copy of this script is running, and how old it is: the ARM64
    # laptop ran one pinned at v0.51.1 for days, found only from a side effect
    # (#201).
    $self = if ($Booted) { (git -C $Work log -1 --format='%h %s' $Booted -- scripts/auto-wintest.ps1 2>$null) -join '' }
            else { (git -C $PSScriptRoot log -1 --format='%h %s' -- auto-wintest.ps1 2>$null) -join '' }
    Say "Script: $PSCommandPath ($self)"
    Say "Model: $Model"
    $outcome = "A run is going (started $('{0:yyyy-MM-dd HH:mm}' -f (Get-Date)) on $head, model $Model)."
    Publish-Status
    $outcome = $null
    $inWork = [IO.Path]::GetFullPath($PSScriptRoot).StartsWith([IO.Path]::GetFullPath($Work), [StringComparison]::OrdinalIgnoreCase)
    if (-not $Booted -and -not $inWork) {
        Say "This script is neither main's copy nor the worktree's, so it does not follow origin/main. Point the task at $Work\scripts\lane-boot.ps1 (see the top of the script)."
    }

    # This run's own scratch folder, and the oldest ones beyond three gone.
    Get-ChildItem -Directory -Path $Scratch -Filter 'run-*' -ErrorAction SilentlyContinue |
        Sort-Object Name -Descending | Select-Object -Skip 2 |
        ForEach-Object { Remove-Item -Recurse -Force -LiteralPath $_.FullName -ErrorAction SilentlyContinue }
    # Leftovers from before the run-* folders: only the two names the tests make, only when old.
    foreach ($pat in 'kura-test-*', 'kura-archive-*') {
        Get-ChildItem -Directory -Path $Scratch -Filter $pat -ErrorAction SilentlyContinue |
            Where-Object { $_.LastWriteTime -lt (Get-Date).AddDays(-3) } |
            ForEach-Object { Remove-Item -Recurse -Force -LiteralPath $_.FullName -ErrorAction SilentlyContinue }
    }
    # The report's name carries the start time: a section run twice on one day
    # (a re-test) wrote the earlier run's file again, which the Merge lanes
    # workflow refuses (a run only adds its own report).
    $started = Get-Date
    $report = 'qa-reports/{0:yyyy-MM-dd}-<ブランチ名から test/ を除いたもの>-{0:HHmm}.md' -f $started
    $Scratch = Join-Path $Scratch ('run-{0:yyyyMMdd-HHmmss}' -f $started)
    New-Item -ItemType Directory -Force -Path $Scratch | Out-Null

    $prompt = "無人実行です。人は見ていません。.claude/windows-role.md を読み、その「Unattended runs」の節に従って、$queue の次の節を 1 つだけ進めてください。レーンは $Lane で、ブランチは test/$Lane-<節> です。チェックアウトは $Work です（役割定義に出てくる C:\dev\kura は、すべてここに読み替えてください）。作業用の一時ディレクトリは $Scratch で、TEMP / TMP も既にそこを指しています（役割定義に出てくる R:\Temp は、すべてここに読み替えてください）。報告は $report に書きます（新しいファイル。既にある報告は直しません）。main の取り込みはしないでください（終わった後にこのスクリプトがします）。"
    $env:TEMP = $Scratch
    $env:TMP = $Scratch
    $env:CARGO_INCREMENTAL = '0'
    Say "Build output: $(Set-BuildTarget)"

    # The desktop, shared with tsumugi's lane (see the top).
    $desktop = [Threading.Mutex]::new($false, 'Local\wintest-desktop')
    $t = Get-Date
    try { $got = $desktop.WaitOne([TimeSpan]::FromMinutes($DesktopWaitMin)) } catch [Threading.AbandonedMutexException] { $got = $true }
    $waited = ((Get-Date) - $t).TotalMinutes
    if (-not $got) {
        $desktop = $null
        Say ("desktop lock: still held after {0:N0} min (tsumugi's run?). Trying again next time." -f $waited)
        exit 0
    }
    if ($waited -ge 0.5) { Say ("desktop lock: waited {0:N0} min" -f $waited) }
    if (Get-Process LogonUI -ErrorAction SilentlyContinue) {
        Say 'The screen locked while waiting. Trying again next time.'
        exit 0
    }

    if (-not $KeepScreenSaver) {
        Suspend-ScreenSaver
        $prompt += " スクリーンセーバーはこのスクリプトが実行の間だけ止めています（起動していれば 5 秒以内に止めます）。"
    }
    # Not a reason to stop: PostMessage and --keys still reach the window. The
    # run is told, so it does not record SendInput rows as having done nothing.
    Start-Sleep -Seconds 1
    $desk = Get-InputDesktop
    Say "Input desktop at the start: $desk"
    if ($desk -ne 'Default') {
        $prompt += " 起動時の入力デスクトップは `"$desk`" で、Default ではありません。SendInput のキーとマウスは届かないので、そういう行は測らずに理由を書いて残し、--keys と PostMessage で進められる行だけを進めてください。"
    }
    # claude writes UTF-8, and PowerShell decodes a native command's output
    # with the console's code page -- CP932 on a Japanese Windows -- so every
    # Japanese line of the run's answer reached the log as mojibake
    # (2026-10-03). Decoded as UTF-8 here, it is written to the log as it was.
    $utf8 = [Text.UTF8Encoding]::new($false)
    [Console]::OutputEncoding = $utf8
    $OutputEncoding = $utf8
    Push-Location $Work
    try {
        $out = claude -p $prompt --model $Model --permission-mode acceptEdits --allowedTools $Tools --disallowedTools $Denied 2>&1 | Out-String
        $code = $LASTEXITCODE
        # A `claude` too old for $Model fails in seconds, every firing: the
        # ARM64 laptop sat at 2.1.278 for ten hours on 2026-10-05 while
        # claude-opus-5-5 needed 2.1.280. Update once and go again.
        if ($code -ne 0 -and $out -match 'does not support this model|or newer is required') {
            Add-Content -Path $log -Value "===== exit=$code`n$out"
            $before = (claude --version 2>&1 | Out-String).Trim()
            $upd = (claude update 2>&1 | Out-String).Trim()
            $after = (claude --version 2>&1 | Out-String).Trim()
            Say "claude was too old for $Model ($before); ran claude update: $(($upd -split "`r?`n")[-1]) Now $after. Trying again."
            $out = claude -p $prompt --model $Model --permission-mode acceptEdits --allowedTools $Tools --disallowedTools $Denied 2>&1 | Out-String
            $code = $LASTEXITCODE
        }
    } finally {
        Pop-Location
        if (-not $KeepScreenSaver) { Resume-ScreenSaver }
    }
    Add-Content -Path $log -Value "===== exit=$code`n$out"

    $tail = ($out.TrimEnd() -split "`r?`n")[-1].Trim()
    # Failures in a row: a lane that fails every firing makes no pull request
    # and says nothing anywhere else, so the owner noticed the ten hours above
    # only by the missing pull requests.
    if ($code -eq 0) {
        # Only a finished run uses the trigger up. A failed one is tried
        # again on the next firing, from the same commit.
        Set-Content -NoNewline -Path $last -Value $trigger
        Remove-Item -LiteralPath $failFile -ErrorAction SilentlyContinue
        Say "Done: $tail"
        $outcome = "Done: $tail"
        # The run's branch, from its pull request: kept mergeable while main moves.
        if ($tail -match '^WINTEST_DONE\s+(\S+)') {
            $branch = (gh pr view $Matches[1] --repo $repo --json headRefName --jq .headRefName 2>$null | Out-String).Trim()
            if ($branch -like "test/$Lane-*") { Sync-LaneBranch $branch } else { Say "Sync: no lane branch for $($Matches[1]); left as it is." }
        }
    } elseif ($out -match 'limit') {
        Say 'Hit a usage limit. Trying again next time.'
    } else {
        $fails = 1 + $(if (Test-Path $failFile) { [int](Get-Content -Raw $failFile) } else { 0 })
        Set-Content -NoNewline -Path $failFile -Value $fails
        Say "The run failed (exit $code). See the log. Last line: $tail"
        $outcome = "The run failed (exit $code), $fails in a row. Last line: $tail"
        if ($fails -ge 3) {
            Say "!!!!! [$Lane] $fails runs in a row have failed. Nothing reaches GitHub until this is fixed. Last line: $tail !!!!!"
        }
        exit 1
    }
} catch {
    $outcome = "The script stopped with an error at line $($_.InvocationInfo.ScriptLineNumber): $_"
    Say $outcome
    throw
} finally {
    Publish-Status
    if ($desktop) { $desktop.ReleaseMutex() }
    $mutex.ReleaseMutex()
}
