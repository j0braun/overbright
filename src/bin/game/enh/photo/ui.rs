use super::*;

impl Game {
    /// The help bar: the selected setting with its neighbours, the
    /// controls, the last picture's name; in the front end's font over a
    /// dark band (the game's help bars' look).
    pub(crate) fn photo_quads(&self) -> Option<Vec<Quad>> {
        let p = self.photo.as_ref()?;
        let mut out = Vec::new();
        if p.capturing || !p.help {
            return Some(out);
        }
        let Some(font) = self
            .game_data.assets
            .fonts
            .get(&nfsu2::formats::hash::bin_hash("CONDUITMDITC_TT21I"))
            .or_else(|| self.game_data.assets.fonts.values().next())
        else {
            return Some(out);
        };
        let band = |y0: f32, y1: f32, a: f32| Quad {
            texture: None,
            rect: [Vec2::new(0.0, y0), Vec2::new(640.0, y1)],
            uv: [Vec2::ZERO, Vec2::ONE],
            color: [0.0, 0.0, 0.0, a],
            z: 2.0,
            angle: 0.0,
            place: nfsu2::render::display::Place::new(0, 1),
        };
        out.push(band(400.0, 480.0, 0.62));
        out.push(Quad { color: [0.98, 0.78, 0.16, 0.9], ..band(400.0, 401.5, 1.0) });
        let white = [0.92, 0.92, 0.92, 1.0];
        let gold = [0.98, 0.80, 0.20, 1.0];
        let grey = [0.62, 0.62, 0.62, 1.0];
        let start = out.len();
        nfsu2::frontend::text_quads(
            &self.game_data.assets,
            font,
            "PHOTO MODE",
            0,
            Vec2::splat(0.62),
            Vec2::new(16.0, 406.0),
            gold,
            1.0,
            &mut out,
        );
        let row = Row::ALL[p.selected];
        let line = format!("<  {}: {}  >", row.name(), ph::value(&p.rig, &p.look, row));
        nfsu2::frontend::text_quads(
            &self.game_data.assets,
            font,
            &line,
            1,
            Vec2::splat(0.62),
            Vec2::new(320.0, 406.0),
            white,
            1.0,
            &mut out,
        );
        let next = Row::ALL[(p.selected + 1) % Row::ALL.len()];
        let side = format!("{}: {}", next.name(), ph::value(&p.rig, &p.look, next));
        nfsu2::frontend::text_quads(
            &self.game_data.assets,
            font,
            &side,
            2,
            Vec2::splat(0.45),
            Vec2::new(624.0, 410.0),
            grey,
            1.0,
            &mut out,
        );
        nfsu2::frontend::text_quads(
            &self.game_data.assets,
            font,
            ph::HELP_KEYS,
            1,
            Vec2::splat(0.42),
            Vec2::new(320.0, 432.0),
            white,
            1.0,
            &mut out,
        );
        nfsu2::frontend::text_quads(
            &self.game_data.assets,
            font,
            ph::HELP_PAD,
            1,
            Vec2::splat(0.42),
            Vec2::new(320.0, 447.0),
            grey,
            1.0,
            &mut out,
        );
        if let Some((text, _)) = &p.status {
            nfsu2::frontend::text_quads(
                &self.game_data.assets,
                font,
                text,
                1,
                Vec2::splat(0.45),
                Vec2::new(320.0, 462.0),
                gold,
                1.0,
                &mut out,
            );
        }
        for q in &mut out[start..] {
            q.place = nfsu2::render::display::Place::new(0, 1);
        }
        Some(out)
    }
}
