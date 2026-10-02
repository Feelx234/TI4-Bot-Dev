@{
    # Full-game self-play PPO from the trade-teacher checkpoint (operator, 2026-09-23: "resume the
    # ppo for like 100 updates, then I'd like to check in replayer").
    # Start: out/trade-teacher-smoke-20260923/checkpoint-20 -- checkpoint-42992-deal-value with the
    # diplomacy readout and deal-value rows trained by supervision on planner-labelled negotiations
    # (commit 313a5e4). Greedy, diplomacy on: 89.85% clearance, 2.87 table VP, 280 diplomacy decisions
    # per game. Same self-play shape and reward flags as the 3000-update run. Fresh seeds.

    Bundle   = 'D:\Projects\ti4-engine-rs\out\trade-teacher-smoke-20260923\checkpoint-20'
    Pool     = 'D:\Projects\ti4-engine-rs\out\pools\full_np8_12_train.json'
    Run      = 'D:\Projects\ti4-engine-rs\out\ppo-armB-waste1-teacher20-100-20260923'
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
        'seed-base'      = 1262300000
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
