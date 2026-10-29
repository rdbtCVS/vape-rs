# HUD helper triage

MouseDelayFix is registered only for 1.8.9 in `ModManager.registerHudModules`. Its event intercepts the local entity's look-vector calculation in `EntityLivingBaseEventMappingTask` / `EventEntityRendererRayTrace`, returning the mapped look-vector bridge result. At the pinned Horizon revision, `Game::mob_hit` computes the attack ray from `view_vector(self.yaw, self.pitch)` and `remote_object_hit` likewise uses current yaw/pitch. The specific interpolated-vector workaround has no separate equivalent to toggle. This is a source review, not a gameplay validation, and does not establish equivalence for every targeting or input behavior.

`ScoreboardVisibleScorePredicate` is a package-private predicate that accepts non-null score owners not beginning with `#`; it is not a separately registered module. Its behavior remains a requirement of the unfinished custom Scoreboard implementation. Marking the helper reviewed does not mark Scoreboard implemented.
