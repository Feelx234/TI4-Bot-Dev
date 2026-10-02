@{
    # SELF-PLAY at 30 seeds x 1 rotation, continued from checkpoint-1560 (update 25) of
    # out/ppo-armB-waste1-selfplay37368-24seeds-20260922 (config armB_waste1_selfplay37368_24seeds.psd1,
    # which continued checkpoint-37368 of the 96-game run). 24 games an update left CPU idle
    # (operator, 2026-09-22: "start this again with 30 seeds"). Seed base moved past the 4,800 seeds
    # the 24-seed run reserved. Reward flags unchanged.

    Bundle   = 'D:\Projects\ti4-engine-rs\out\ppo-armB-waste1-selfplay37368-24seeds-20260922\checkpoints\checkpoint-1560'
    Pool     = 'D:\Projects\ti4-engine-rs\out\pools\full_np8_12_train.json'
    Run      = 'D:\Projects\ti4-engine-rs\out\ppo-armB-waste1-selfplay1560-30seeds-20260922'
    LibTorch = 'D:\Projects\ti4-engine-rs\out\libtorch-2.9.1-cu128'

    Diplomacy       = $true
    Background      = $true
    Build           = $true
    AllowConcurrent = $false

    Flags = @{
        'stage'          = 2
        'rounds'         = 4
        'temperature'    = 2.5
        'learning-rate'  = '1e-4'
        'movement-entropy' = 0.05
        'entropy-final'  = 0.25
        'updates'        = 200
        'report-every'   = '1:1,25:250,100'
        'seed-base'      = 1262020000
        'seeds-per-update' = 30
        'rotations'      = 1
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
