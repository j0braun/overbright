use super::*;

/// 0x4C66F0(bar, v, 1, top, bottom): a vertical bar over the image's
/// texture height H (texture +0x46). f = 0x4A9D60(0, 1, (H − top)/H,
/// bottom/H, v), the linear map clamped at both ends, becomes the top
/// texture coordinate (object +0x48) and the height (1 − f)·H; the
/// position moves by half the height change, so the bottom edge stays put
/// (0x517DB0, 0x517C10). HEAT_Bar takes (105, 5), NOS_Line (45, −9).
pub(super) fn bar(
    assets: &Assets,
    package: &super::super::feng::Package,
    s: &mut ScreenState,
    name: &str,
    heat: f32,
    top: f32,
    bottom: f32,
) {
    let bar = fe_hash(name);
    let Some(o) = package.objects.iter().find(|o| o.hash == bar) else { return };
    let Some(t) = package
        .resources
        .get(o.resource as usize)
        .and_then(|r| assets.texture_for(&r.name))
        .and_then(|h| assets.textures.get(&h))
    else {
        return;
    };
    let h = t.height as f32;
    if h <= 0.0 {
        return;
    }
    let (lo, hi) = ((h - top) / h, bottom / h);
    let f = if heat <= 0.0 {
        lo
    } else if heat >= 1.0 {
        hi
    } else {
        lo + heat * (hi - lo)
    };
    let a = &o.attributes;
    let mut uv = a.uv();
    uv[1] = f;
    s.uv.insert(bar, uv);
    let (size, at) = (a.size(), a.position());
    let height = (1.0 - f) * h;
    s.size.insert(bar, glam::Vec2::new(size.x, height));
    s.position.insert(bar, glam::Vec2::new(at.x, (size.y - height) * 0.5 + at.y));
}

/// 0x4C67E0(bar, v, 1, 99, 1): a horizontal bar over the image's texture
/// width W (texture +0x44). f = 0x4A9D60(0, 1, (W − 99)/W, 1/W, v), the
/// clamped linear map, gives the right texture coordinate 1 − f (object
/// +0x4C) and the width (1 − f)·W; the position moves by half the width
/// change, so the left edge stays put (0x517DB0, 0x517C10). The outrun
/// meter's Getaway_Distance_Bar takes it.
pub(super) fn hbar(assets: &Assets, package: &super::super::feng::Package, s: &mut ScreenState, name: &str, v: f32) {
    let bar = fe_hash(name);
    let Some(o) = package.objects.iter().find(|o| o.hash == bar) else { return };
    let Some(t) = package
        .resources
        .get(o.resource as usize)
        .and_then(|r| assets.texture_for(&r.name))
        .and_then(|h| assets.textures.get(&h))
    else {
        return;
    };
    let w = t.width as f32;
    if w <= 0.0 {
        return;
    }
    let (lo, hi) = ((w - 99.0) / w, 1.0 / w);
    let f = if v <= 0.0 {
        lo
    } else if v >= 1.0 {
        hi
    } else {
        lo + v * (hi - lo)
    };
    let a = &o.attributes;
    let mut uv = a.uv();
    uv[2] = 1.0 - f;
    s.uv.insert(bar, uv);
    let (size, at) = (a.size(), a.position());
    let width = (1.0 - f) * w;
    s.size.insert(bar, glam::Vec2::new(width, size.y));
    s.position.insert(bar, glam::Vec2::new(at.x - (size.x - width) * 0.5, at.y));
}
