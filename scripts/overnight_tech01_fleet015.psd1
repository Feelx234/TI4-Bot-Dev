@{
    # Overnight self-play PPO (operator, 2026-09-23: "set up overnight training, tech down to 0.1 and
    # add a fleet term, but not as big as it historically was, maybe half").
    # Start: checkpoint-7592, the end of the tech-weight 0.25 run (3.02 technologies per seat, up
    # from 0.13; greedy clearance 85.9%, down from 90.2%). tech-weight 0.1 keeps research paid for with
    # less pull away from clearance; fleet-weight 0.015 is half the 0.03 used before. Everything else
    # unchanged. ~16 s per update, 1800 updates is about eight hours. Fresh seeds.

    Bundle   = 'D:\Projects\ti4-engine-rs\out\ppo-armB-waste1-tech025-6352-100-20260923\checkpoints\checkpoint-7592'
    Pool     = 'D:\Projects\ti4-engine-rs\out\pools\full_np8_12_train.json'
    Run      = 'D:\Projects\ti4-engine-rs\out\ppo-armB-waste1-overnight-tech01-fleet015-20260923'
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
        'updates'        = 1800
        'report-every'   = '1:1,25:250,100'
        'seed-base'      = 1262500000
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
        'tech-weight'                 = 0.1
        'fleet-weight'                = 0.015
        'fleet-hoard-penalty'         = 0
        'zero-fleet-penalty'          = 0
        'trade-goods-hoard-weight'    = 0
        'strategy-diversity-weight'   = 0
        'styx-bonus'                  = 0
        'fracture-entry-bonus'        = 0
        'fracture-planet-bonus'       = 0
    }
}
