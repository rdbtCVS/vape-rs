// The published mixin tool can't add a field to Game<'_>, so each thread owns
// one runtime. Only the window thread's client Game uses it: hooks return
// before using it on a server or dedicated Game, whose thread drops its unused
// runtime when it ends.
thread_local! {
    static CLIENT: crate::game::vape_rs::core::Client = ::core::default::Default::default();
}

impl crate::game::Game<'_> {
    pub(crate) fn __horizon_mixins_vape_rs_client(&self) -> crate::game::vape_rs::core::Client {
        CLIENT.with(::std::clone::Clone::clone)
    }
}
