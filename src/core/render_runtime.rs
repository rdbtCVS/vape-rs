// The render modules' per-frame entry points that are not world lines: the
// HUD-time overlay (ESP 2D and the other screen-space modules) and ESP's
// Outline colours.
use super::super::modules::render::{
    arrows, center_stack, esp, explosions, health, name_tags, overlay, search, spawner_finder,
};
use super::super::targeting::entities::EntityView;
use super::{ModuleId, Runtime};

impl Runtime {
    /// Whether any overlay module draws this frame.
    pub fn overlay_active(&self) -> bool {
        [
            ModuleId::Esp,
            ModuleId::Health,
            ModuleId::Arrows,
            ModuleId::NameTags,
            ModuleId::SpawnerFinder,
            ModuleId::MurderFinder,
        ]
        .into_iter()
        .any(|id| self.enabled(id))
            || self.totem_count_shown()
    }
    /// The friends' aliases, by name, for labels that show them.
    fn friend_aliases(&self) -> Vec<(String, String)> {
        self.document.0["friends"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|p| {
                let name = p["name"].as_str()?;
                let alias = p["alias"].as_str().filter(|a| !a.is_empty())?;
                Some((name.to_owned(), alias.to_owned()))
            })
            .collect()
    }
    /// One frame's overlay draws, in paint order.
    pub fn overlay(&mut self, frame: &overlay::Frame) -> Vec<overlay::Draw> {
        let mut out = Vec::new();
        let people = self.target_people();
        if self.enabled(ModuleId::Esp) {
            out.extend(esp::esp_2d(
                &self.settings,
                frame,
                &people,
                &self.friend_aliases(),
            ));
        }
        if self.enabled(ModuleId::NameTags) {
            out.extend(name_tags::equipment_row(&self.settings, frame, &people));
        }
        if self.enabled(ModuleId::Arrows) {
            out.extend(arrows::draws(&self.settings, frame, &people));
        }
        let mut stack = Vec::new();
        if self.enabled(ModuleId::Health) {
            stack.push(health::entry(frame));
        }
        if self.totem_count_shown() {
            stack.push(center_stack::totem_entry(frame.own.totems));
        }
        out.extend(center_stack::draws(frame, &stack));
        if self.enabled(ModuleId::SpawnerFinder) {
            out.extend(spawner_finder::draws(&self.settings, frame));
        }
        if self.enabled(ModuleId::MurderFinder) {
            let epoch = self.session.epoch();
            self.murder_finder.observe(&self.settings, epoch, frame);
            out.extend(self.murder_finder.draws(frame));
        }
        out
    }
    /// ESP Outline's glowing colours (RGB) by entity id.
    pub fn esp_outlines(&self, entities: &[EntityView]) -> Vec<(i32, u32)> {
        if !self.enabled(ModuleId::Esp) {
            return Vec::new();
        }
        let people = self.target_people();
        entities
            .iter()
            .filter_map(|e| Some((e.handle.id, esp::outline_color(&self.settings, e, &people)?)))
            .collect()
    }
}

impl Runtime {
    /// Whether any module fills world quads this frame.
    pub fn world_quads_active(&self) -> bool {
        self.enabled(ModuleId::StorageEsp)
            || self.enabled(ModuleId::Explosions)
            || self.enabled(ModuleId::PropHunt)
    }
    /// The through-wall filled quads of StorageESP and Explosions for one
    /// frame.
    pub fn world_quads(
        &self,
        storage: &[super::super::modules::render::storage_esp::Block],
        tnts: &[explosions::Tnt],
        props: &[[f64; 3]],
        partial: f32,
    ) -> Vec<super::super::modules::render::world::Quad> {
        let mut out = Vec::new();
        if self.enabled(ModuleId::StorageEsp) {
            super::super::modules::render::storage_esp::quads(&self.settings, storage, &mut out);
        }
        if self.enabled(ModuleId::Explosions) {
            explosions::quads(tnts, partial, &mut out);
        }
        if self.enabled(ModuleId::PropHunt) {
            self.prop_hunt.quads(props, &mut out);
        }
        out
    }
    /// Explosions' blast rings (depth-tested world lines).
    pub fn explosion_lines(
        &self,
        tnts: &[explosions::Tnt],
        partial: f32,
    ) -> Vec<super::super::modules::render::world::Line> {
        let mut out = Vec::new();
        if self.enabled(ModuleId::Explosions) {
            explosions::lines(&self.settings, tnts, partial, &mut out);
        }
        out
    }
    /// Search's scan request, when it is on and has blocks to find.
    pub fn search_query(&self) -> Option<search::Query> {
        if !self.enabled(ModuleId::Search) {
            return None;
        }
        let query = search::query(&self.settings);
        (!query.blocks.is_empty()).then_some(query)
    }
    /// Whether the host should gather spawners (SpawnerFinder) or falling
    /// blocks (PropHunt) this frame.
    pub fn spawners_wanted(&self) -> bool {
        self.enabled(ModuleId::SpawnerFinder)
    }
    pub fn props_wanted(&self) -> bool {
        self.enabled(ModuleId::PropHunt)
    }
    /// PropHunt on a single block update packet.
    pub fn prop_hunt_block_update(&mut self, pos: [i32; 3]) {
        if self.enabled(ModuleId::PropHunt) {
            self.prop_hunt.block_update(pos);
        }
    }
    /// The through-wall lines of Search and PropHunt for one frame.
    pub fn scene_lines(
        &self,
        camera: &esp::Camera,
        found: &[([i32; 3], String)],
        props: &[[f64; 3]],
    ) -> Vec<super::super::modules::render::world::Line> {
        let mut out = Vec::new();
        if self.enabled(ModuleId::Search) {
            search::lines(&self.settings, camera, found, &mut out);
        }
        if self.enabled(ModuleId::PropHunt) {
            self.prop_hunt.lines(props, &mut out);
        }
        out
    }
}
