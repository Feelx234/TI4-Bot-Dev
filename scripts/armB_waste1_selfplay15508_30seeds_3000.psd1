@{
    # SELF-PLAY at 30 seeds x 1 rotation, 3000 updates, continued from checkpoint-15508 (the final
    # checkpoint, update 200) of out/ppo-armB-waste1-selfplay1560-30seeds-20260922
    # (operator, 2026-09-22: "resume training for 3000 updates"). Near-greedy eval of 15508:
    # self-play table 2.65 VP vs 2.29 for checkpoint-2196-deal-vocab; one seat against five 2196
    # seats 2.95 VP. Seed base moved past the 6,000 seeds the previous run used. Reward flags unchanged.

    Bundle   = 'D:\Projects\ti4-engine-rs\out\ppo-armB-waste1-selfplay1560-30seeds-20260922\checkpoints\checkpoint-15508'
    Pool     = 'D:\Projects\ti4-engine-rs\out\pools\full_np8_12_train.json'
    Run      = 'D:\Projects\ti4-engine-rs\out\ppo-armB-waste1-selfplay15508-30seeds-3000-20260922'
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
        'updates'        = 3000
        'report-every'   = '1:1,25:250,100'
        'seed-base'      = 1262030000
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
