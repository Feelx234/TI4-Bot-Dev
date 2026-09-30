@{
    # SELF-PLAY, continued from checkpoint-37368 (update ~150) of
    # out/ppo-armB-waste1-selfplay2196-dealvocab-20260922, stopped at update 159 because its optimise
    # step had slowed from ~21 s to 70-110 s per update at 96 games (operator, 2026-09-22).
    #
    # 24 seeds x 1 rotation per update instead of 16 x 6: in self-play every seat trains in every game,
    # so rotations bought only six correlated games on one map; 24 seeds give 24 maps and seatings for
    # a quarter of the games (operator: "restart like that from latest checkpoint").
    # Seed base moved past the 3,200 seeds the source run had reserved.
    #
    # Engine/policy since the source run: 7f3ccad sends the deal builder's decisions (item, amount,
    # review) to the diplomacy head instead of the catch-all (operator: "include the change").
    # Reward flags unchanged.

    Bundle   = 'D:\Projects\ti4-engine-rs\out\ppo-armB-waste1-selfplay2196-dealvocab-20260922\checkpoints\checkpoint-37368'
    Pool     = 'D:\Projects\ti4-engine-rs\out\pools\full_np8_12_train.json'
    Run      = 'D:\Projects\ti4-engine-rs\out\ppo-armB-waste1-selfplay37368-24seeds-20260922'
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
        'seed-base'      = 1262010000
        'seeds-per-update' = 24
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
