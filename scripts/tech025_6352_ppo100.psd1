@{
    # Self-play PPO with a technology reward (operator, 2026-09-23: "run ppo with tech weight").
    # Start: checkpoint-6352, the end of the 100-update run from the trade-teacher checkpoint, which
    # the operator reviewed ("hacan is paying relatively fair prices for support right now").
    # Why: that checkpoint researches 0.13 technologies per seat in four rounds (42992: 1.08) and
    # never drafts Technology (0 of 480 first-seat picks; 42992: 27). tech-weight 0.25 pays a
    # quarter point per technology gained, below vp-weight 1. Everything else as before. Fresh seeds.

    Bundle   = 'D:\Projects\ti4-engine-rs\out\ppo-armB-waste1-teacher20-100-20260923\checkpoints\checkpoint-6352'
    Pool     = 'D:\Projects\ti4-engine-rs\out\pools\full_np8_12_train.json'
    Run      = 'D:\Projects\ti4-engine-rs\out\ppo-armB-waste1-tech025-6352-100-20260923'
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
        'updates'        = 100
        'report-every'   = '1:1,25:250,100'
        'seed-base'      = 1262400000
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
        'tech-weight'                 = 0.25
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
