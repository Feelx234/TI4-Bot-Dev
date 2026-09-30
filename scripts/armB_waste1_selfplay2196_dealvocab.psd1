@{
    # SELF-PLAY from checkpoint-2196-deal-vocab (operator, 2026-09-22).
    #
    # The frozen-opponent run (scripts/armB_waste1_resume2196_dealvocab.psd1) exploited its
    # opponents: at update 225, 2.97 of the learner's 5.14 VP per game came from Support for the
    # Throne notes the near-greedy frozen policy handed over. Against itself that checkpoint scored
    # 2.96 table VP, not 6.2. This run drops --opponent: every seat plays with the current weights
    # and every seat's decisions train (~245k decisions, ~48 s per update), so giving a Support note
    # away costs the giver. 200 updates (~2.7 h; operator, 2026-09-22).
    #
    # Everything else is the frozen-opponent run's config, which copied the reward flags of
    # out/ppo-activation-armB-waste1-resume3224-20260920 unchanged.

    Bundle   = 'D:\Projects\ti4-engine-rs\out\ppo-activation-armB-waste1-resume3224-20260920\checkpoints\checkpoint-2196-deal-vocab'
    Pool     = 'D:\Projects\ti4-engine-rs\out\pools\full_np8_12_train.json'
    Run      = 'D:\Projects\ti4-engine-rs\out\ppo-armB-waste1-selfplay2196-dealvocab-20260922'
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
