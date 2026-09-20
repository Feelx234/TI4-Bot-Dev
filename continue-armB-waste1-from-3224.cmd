@echo off
setlocal
rem -------------------------------------------------------------------------
rem  Continue PPO training from an existing checkpoint, same reward terms.
rem
rem  Warm start : out\ppo-activation-armB-waste1-20260918\checkpoints\checkpoint-3224
rem  Reward     : vp-weight 1, waste-penalty 1, clearance-weight 0.5,
rem               r1-bonus 3, r1-shaping 0.1, everything else zero.
rem               Copied from that run's launch.json, not retyped from memory.
rem  New output : out\ppo-activation-armB-waste1-resume3224-20260920
rem
rem  The trainer starts detached and writes stdout.log, stderr.log and pid.txt
rem  into the new run directory. It keeps running if you close this window.
rem
rem  Preview without building or starting anything:
rem      continue-armB-waste1-from-3224.cmd -DryRun
rem -------------------------------------------------------------------------
cd /d "%~dp0"

echo Continuing PPO from checkpoint-3224 (arm B, waste 1).
echo Reward terms are unchanged; output goes to a new dated run directory.
echo.

powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\ppo_train.ps1" -Config "%~dp0scripts\activation_armB_waste1_resume3224.psd1" %*
set EC=%ERRORLEVEL%

if not "%EC%"=="0" echo. 1>&2 && echo ppo_train.ps1 exited with code %EC% 1>&2
exit /b %EC%
