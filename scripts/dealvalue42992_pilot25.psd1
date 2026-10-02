@{
    # DEAL VALUES, 25-update pilot (operator, 2026-09-23: "try doing it like the battle predictor").
    # checkpoint-42992-deal-value is checkpoint-42992 (update 515 of the 3000-update self-play run)
    # with three names appended into zero rows: diplomacy:deal-value-own / -partner / -score. The
    # bot puts the value sheet's numbers (plans/TRADE_ARENA_VALUE_SHEET_2026-09-22.md, alpha 0.75)
    # on every diplomacy option as those facts, the way battle facts sit on movement options; PPO
    # learns how much to trust them. At zero rows it plays exactly like 42992 (greedy eval, diplomacy
    # on: 89.03% clearance, 3.685 table VP, identical per faction).
    # Same self-play shape and reward flags as the 3000-update run. Seeds past everything used so far.

    Bundle   = 'D:\Projects\ti4-engine-rs\out\ppo-armB-waste1-selfplay15508-30seeds-3000-20260922\checkpoints\checkpoint-42992-deal-value'
    Pool     = 'D:\Projects\ti4-engine-rs\out\pools\full_np8_12_train.json'
    Run      = 'D:\Projects\ti4-engine-rs\out\ppo-armB-waste1-dealvalue42992-pilot25-20260923'
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
        'updates'        = 25
        'report-every'   = '1:1,25:250,100'
        'seed-base'      = 1262200000
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
