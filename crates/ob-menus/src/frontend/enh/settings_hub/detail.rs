use super::*;

// ---------------------------------------------------------------------
// The detail line.

fn origin_text(id: RowId, r: Option<&GraphicsRow>, s: &Settings) -> &'static str {
    let origin = settings_model::model(id).map(|m| m.origin);
    match (origin, r) {
        (_, _) if matches!(id, RowId::Custom(_)) => text(Text::OriginNative),
        (Some(Origin::Native), _) => text(Text::OriginNative),
        (Some(Origin::Shared(_)), Some(r)) if r.enhanced(s) => text(Text::OriginSharedEnhanced),
        (Some(Origin::Shared(_)), _) => text(Text::OriginSharedNative),
        (Some(Origin::Utility), _) => text(Text::OriginUtility),
        (Some(Origin::Enhanced), _) if !changed(id, s) => text(Text::OriginEnhancedOff),
        _ => text(Text::OriginEnhanced),
    }
}

fn applies_text(id: RowId, r: Option<&GraphicsRow>, s: &Settings) -> String {
    match settings_model::model(id).map(|m| m.applies) {
        Some(Applies::NextLoad(what)) => fill(Text::AppliesNextLoad, &[what]),
        Some(Applies::Restart) => text(Text::AppliesRestart).to_string(),
        Some(Applies::RestartFor(tiers)) if r.is_some_and(|r| tiers.contains(&(r.get)(s))) => {
            text(Text::AppliesRestartTier).to_string()
        }
        _ => text(Text::AppliesLive).to_string(),
    }
}

fn cost_text(id: RowId, s: &Settings) -> String {
    let out = qol::output_size_or(s.resolution);
    let (Some(at_4k), Some(here)) = (options::cost_ms(id, s, (3840, 2160)), options::cost_ms(id, s, out)) else {
        return text(Text::CostNone).to_string();
    };
    let size = format!("{}x{}", out.0, out.1);
    if at_4k <= -0.05 {
        fill(Text::SavesAt, &[&format!("{:.2}", -at_4k), &format!("{here:.2}"), &size])
    } else if at_4k < 0.05 {
        text(Text::CostFree).to_string()
    } else {
        fill(Text::CostAt, &[&format!("{at_4k:.2}"), &format!("{here:.2}"), &size])
    }
}

/// The detail line of row `id` on the page's `category`.
pub fn detail(category: Category, id: RowId, s: &Settings) -> Option<String> {
    if let Some(line) = main_detail(category, id, s) {
        return Some(line);
    }
    let line = match id {
        RowId::GraphicsPreset => preset_help(s),
        RowId::UxCompare => text(Text::HubCompare).to_string(),
        RowId::UxFilter => text(Text::HubShow).to_string(),
        RowId::UxGroup(g) => {
            let ids = group_ids(g);
            let n = ids.iter().filter(|&&id| changed(id, s)).count();
            format!("{} {n} of {} rows changed from the original. {}", g.help(), ids.len(), text(Text::TabsHint))
        }
        RowId::RestoreNative => text(Text::HubRestore).to_string(),
        RowId::UxReset(_) => text(Text::ResetGroup).to_string(),
        RowId::UxNoMatch => format!("{} {}", text(Text::HubShow), text(Text::TabsHint)),
        RowId::Custom(c) => {
            let need: Vec<&str> = settings_model::RULES
                .iter()
                .filter(|r| r.by == RowId::Custom(c))
                .filter_map(|r| options::clone_label(graphics_table::row(r.row)?.label))
                .collect();
            format!("{} {} Needed by: {}.", text(Text::OriginNative), text(Text::NativeRow), need.join(", "))
        }
        _ => {
            let r = graphics_table::row(id);
            let mut parts: Vec<String> = Vec::new();
            // The status first: origin, when it applies, the cost.
            let mut status = format!("{} {}", origin_text(id, r, s), applies_text(id, r, s));
            let cost = cost_text(id, s);
            if !cost.is_empty() {
                status.push(' ');
                status.push_str(&cost);
            }
            parts.push(status);
            // The rule in force (overridden, needs, blocked).
            if let (Some(rule), Some(name)) =
                (settings_model::rule_for(id, s), r.and_then(|r| r.tiers.get((r.get)(s))).map(|t| t.name))
            {
                let shown = settings_model::shown(id, name, s).map_or(String::new(), |(t, _)| t);
                parts.push(if shown.is_empty() || shown.starts_with(name) {
                    rule.reason.to_string()
                } else {
                    format!("{shown}: {}", rule.reason)
                });
            }
            match r {
                Some(r) => {
                    parts.push(r.help.to_string());
                    if let Some(t) = r.tiers.get((r.get)(s)).filter(|t| !t.help.is_empty()) {
                        parts.push(t.help.to_string());
                    }
                }
                None => parts.extend(options::detail(Category::Graphics, id, s)),
            }
            parts.join(" ")
        }
    };
    let _ = category;
    Some(line.trim().to_string())
}
