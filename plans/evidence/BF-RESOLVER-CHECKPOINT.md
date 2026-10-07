# BF-RESOLVER-CHECKPOINT

P1 foundation dependency for transactional BF timing routes. Writable source timing.rs and this evidence; no external access, new folders, deletion, or artifacts beyond ignored Cargo output. Objective: checkpoint resolver registration, order/configuration, lifecycle/frequency use, resolver/application trace and owned decision log; restoring must preserve consumed decider answers. Contextual TimingContext.table.log remains caller responsibility.

Independent Terra reviewer found the inherited API omitted owned Table.log and misattached Resolver documentation. Root added DecisionLog to the snapshot/restore and documented external contextual log responsibility. Added standalone failed stateful emission regression: restore exact replay log, then next emission receives the next decline instead of replaying the consumed answer. Focused result/review follow-up pending. No isolated committed-tree build claimed. Stage only this foundation once checks/review pass, before dependent Sardakk/Mentak/shared timing fixes.

Focused `cargo test -p ti4-engine --lib checkpoint_restores_standalone -q -j8 -- --test-threads=8`: **1 passed**, zero failures,2043 filtered. Independent Terra rereview reports no findings and recommends acceptance; reviewer ran no tests. Coordinated final full engine and lint pending; priorfullfailures isolated to new Yinfixtures.

Final coordinated full shared-tree run `cargo test -p ti4-engine -q -j8 -- --test-threads=8`: **2043 library passed, zero failures, one ignored**; integrations1/1, decision inventory4/4, other5/5; doctests passed. All new route and failure fixtures passed. No isolated committed-tree build or full BF completion claim.

`cargo clippy -p ti4-engine --lib -q -j8` exited0 with existing/shared-WIP warnings; not warning-clean. Independent Terra final rereview reports no remaining findings.
