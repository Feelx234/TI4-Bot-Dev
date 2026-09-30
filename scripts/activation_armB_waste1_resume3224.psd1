@{
    # Continue the arm B (packages) waste-1 run from its own checkpoint 3224, with the reward
    # function unchanged.
    #
    # Every flag below was copied out of the recorded launch of the run being resumed:
    #
    #   out/ppo-activation-armB-waste1-20260918/launch.json
    #     started_at  2026-09-18T09:45:58+02:00
    #     git_commit  bc7a8e17b9b5cc55f825c63979f4b776701dfc2c
    #     config      scripts/activation_armB_waste1.psd1  (in a worktree that no longer exists)
    #
    # Since that commit crates/ti4-mlp has NOT changed, so the trainer and its reward code are the
    # code that produced checkpoint-3224. crates/ti4-engine has changed by two commits: the Guild
    # Ships legality fix (09409d7, which makes Hacan transactions actually resolve, so self-play
    # sees different transaction outcomes than the original run did) and a test-only prerequisite
    # guard (cf14895). That difference is the honest caveat on comparability, not the flags.
    #
    # THE REWARD TERMS, unchanged from the run being resumed:
    #   vp-weight 1, waste-penalty 1, clearance-weight 0.5, r1-bonus 3, r1-shaping 0.1,
    #   and every other term explicitly zero (objective, secret, tech, fleet, fleet-hoard,
    #   zero-fleet, trade-goods-hoard, strategy-diversity, styx, fracture-entry, fracture-planet).
    #
    # What differs from that launch is exactly two things: --bundle points at checkpoint-3224 of this
    # run instead of checkpoint-2548 of the sibling waste run, and --out is a new run directory
    # because ppo_train.ps1 refuses to launch into a directory that already exists.
    #
    # seed-base is kept identical on purpose: same game seeds, so the warm start sees the same seed
    # sequence as the run it came from. Change it here if you want fresh matchups instead. 'updates'
    # is also 1200, which is a fresh block of 1200 updates from the checkpoint, not 1200 total.
    #
    #   .\continue-armB-waste1-from-3224.cmd -DryRun                    (preview, starts nothing)
    #   .\continue-armB-waste1-from-3224.cmd                          (build and start)
    #
    # Background = $true matches the original run: the trainer detaches, writes stdout.log,
    # stderr.log and pid.txt into the run directory, and survives this window closing. Follow it with
    #   Get-Content out\ppo-activation-armB-waste1-resume3224-20260920\stdout.log -Wait
    # Set Background = $false to keep it in the console window instead.

    Bundle   = 'D:\Projects\ti4-engine-rs\out\ppo-activation-armB-waste1-20260918\checkpoints\checkpoint-3224'
    Pool     = 'D:\Projects\ti4-engine-rs\out\pools\full_np8_12_train.json'
    Run      = 'D:\Projects\ti4-engine-rs\out\ppo-activation-armB-waste1-resume3224-20260920'
    LibTorch = 'D:\Projects\ti4-engine-rs\out\libtorch-2.9.1-cu128'

    Diplomacy       = $true
    Background      = $true
    Build           = $true
    AllowConcurrent = $false

    Flags = @{
        # Five frozen copies of the starting policy; one rotating faction learns per game.
        'opponent' = 'D:\Projects\ti4-engine-rs\out\ppo-diplomacy-my-run\checkpoints\checkpoint-212544'

        'stage'          = 2
        'rounds'         = 4
        'temperature'    = 2.5
        'learning-rate'  = '1e-4'
        'movement-entropy' = 0.05
        'entropy-final'  = 0.25
        'updates'        = 1200
        'report-every'   = '1:1,25:250,100'
        'seed-base'      = 1262000000
        'device'         = 'cuda'

        # The objective, and the three shaping terms this arm was running.
        'vp-weight'        = 1
        'waste-penalty'    = 1
        'clearance-weight' = 0.5
        'r1-bonus'         = 3
        'r1-shaping'       = 0.1

        # Explicitly zero, as the recorded launch had them. Stating each one means this file records
        # the control instead of leaving defaults to be inferred later.
        'objective-weight'            = 0
        'secret-weight'               = 0
        'tech-weight'                 = 0
        'fleet-weight'                = 0
        'fleet-hoard-penalty'         = 0
        'zero-fleet-penalty'          = 0
        'trade-goods-hoard-weight'    = 0
        'strategy-diversity-weight'   = 0
        'styx-bonus'                  = 0
        'fracture-entry-bonus'        = 0
        'fracture-planet-bonus'       = 0
    }
}
