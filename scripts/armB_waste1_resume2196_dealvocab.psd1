@{
    # Continue the arm B (packages) waste-1 line from checkpoint-2196 of
    # out/ppo-activation-armB-waste1-resume3224-20260920, on the 2026-09-22 engine, with the reward
    # function unchanged (operator request, 2026-09-22: "with the same reward settings").
    #
    # Every flag is copied from out/ppo-activation-armB-waste1-resume3224-20260920/launch.json
    # (config scripts/activation_armB_waste1_resume3224.psd1, git 90afe71).
    #
    # What differs, and why:
    #   Bundle -- checkpoint-2196-deal-vocab, not checkpoint-2196 itself. It is checkpoint-2196 with
    #     3,410 feature names appended into its preallocated zero rows (tools vocab_census and
    #     migrate_bundle_append_names, commit ed111cc): the deal builder, end-turn and mid-action
    #     pause decisions, the newly implemented techs, and relationship features the old engine
    #     never produced. Rows are zero, so the policy starts out playing exactly like 2196.
    #   Run -- a new directory; ppo_train.ps1 refuses to launch into one that exists.
    #   The engine -- this branch (wp/operator-bugs-2026-09-21): item-by-item deals, explicit end of
    #     turn, negotiation pauses and limits, the faction/generic technology implementations, the
    #     attachment and Direct Hit fixes. Games are not comparable to the source run's; the reward
    #     flags are.
    #
    # THE REWARD TERMS, unchanged:
    #   vp-weight 1, waste-penalty 1, clearance-weight 0.5, r1-bonus 3, r1-shaping 0.1, every other
    #   term explicitly zero.

    Bundle   = 'D:\Projects\ti4-engine-rs\out\ppo-activation-armB-waste1-resume3224-20260920\checkpoints\checkpoint-2196-deal-vocab'
    Pool     = 'D:\Projects\ti4-engine-rs\out\pools\full_np8_12_train.json'
    Run      = 'D:\Projects\ti4-engine-rs\out\ppo-armB-waste1-resume2196-dealvocab-20260922'
    LibTorch = 'D:\Projects\ti4-engine-rs\out\libtorch-2.9.1-cu128'

    Diplomacy       = $true
    Background      = $true
    Build           = $true
    AllowConcurrent = $false

    Flags = @{
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

        'vp-weight'        = 1
        'waste-penalty'    = 1
        'clearance-weight' = 0.5
        'r1-bonus'         = 3
        'r1-shaping'       = 0.1

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
