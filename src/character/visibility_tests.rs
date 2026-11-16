use super::*;
fn model(weights: [f32;3]) -> SkinnedModel {
    SkinnedModel { hash:0,name:String::new(),textures:vec![0],
        vertices: [Vec3::new(-1.0,-2.0,-1.0),Vec3::new(1.0,2.0,1.0)].into_iter().map(|position|
            SkinVertex {position,normal:Vec3::Z,weights,bones:[0,1,255],..Default::default()}).collect(),
        indices:vec![0,1],ranges:vec![SkinRange {texture_slot:0,first_index:0,index_count:2,palette:[0;19]}] }
}
#[test]
fn conservative_skin_bounds_handle_weights_missing_bones_and_nonfinite_inputs() {
    for weights in [[1.0,0.0,0.0],[2.0,0.5,0.0],[0.0;3],[-1.0,0.5,0.5]] {
        let model=model(weights);
        for palette in [vec![],vec![Mat4::from_scale_rotation_translation(Vec3::new(-1.2,0.5,2.0),Quat::from_rotation_z(0.7),Vec3::new(3.0,4.0,5.0))]] {
            let world=Mat4::from_scale_rotation_translation(Vec3::new(0.7,2.0,-1.0),Quat::from_rotation_y(0.3),Vec3::new(1000.0,-2000.0,30.0));
            let b=SkinBounds::build(&model).unwrap().posed(&palette,world).unwrap();
            assert!(model.skin(&palette,world).iter().all(|(p,_,_,_)| b.contains(*p)));
        }
    }
    assert!(SkinBounds::build(&model([f32::NAN,0.0,0.0])).is_none());
    let bound=SkinBounds::build(&model([1.0,0.0,0.0])).unwrap();
    assert!(bound.posed(&[Mat4::from_cols_array(&[f32::NAN;16])],Mat4::IDENTITY).is_none());
    assert!(bound.posed(&[],Mat4::from_cols_array(&[f32::INFINITY;16])).is_none());
    let edge=PosedBounds { min:Vec3::new(1.0,-0.5,-0.5),max:Vec3::new(2.0,0.5,0.5) };
    assert!(edge.visible(Mat4::IDENTITY),"touching the clipping plane stays visible");
    let outside=PosedBounds {min:edge.min+Vec3::X*0.1,max:edge.max+Vec3::X*0.1};
    assert!(!outside.visible(Mat4::IDENTITY));
    assert!(outside.visible(Mat4::from_cols_array(&[f32::NAN;16])));
}
#[test]
fn bounds_keep_camera_intersections_and_finite_cancellation() {
    let around_eye=PosedBounds {min:Vec3::splat(-1.0),max:Vec3::splat(1.0)};
    let view=Mat4::look_at_rh(Vec3::ZERO,Vec3::Y,Vec3::Z);
    assert!(around_eye.visible(Mat4::perspective_rh(0.9,1.7,0.1,100.0)*view));
    let crossing=PosedBounds {min:Vec3::new(0.8,-0.1,-0.1),max:Vec3::new(1.2,0.1,0.1)};
    assert!(crossing.visible(Mat4::IDENTITY));
    assert!(PosedBounds {min:Vec3::splat(f32::NAN),max:Vec3::ONE}.visible(Mat4::IDENTITY));
    let mut model=model([0.33333334,0.6666667,0.0]);
    for (i,v) in model.vertices.iter_mut().enumerate() {
        v.position=Vec3::new(1.0e15+i as f32*1.0e8,1.0e15,1.0);
    }
    let matrix=Mat4::from_cols(Vec3::new(1.0e10,-1.0e10,1.0e10).extend(0.0),
        Vec3::new(-1.0e10,1.0e10,-1.0e10).extend(0.0),Vec3::Z.extend(0.0),
        Vec3::new(1.0e25,-1.0e25,0.0).extend(1.0));
    let world=Mat4::from_translation(Vec3::new(-1.0e25,1.0e25,0.0));
    let b=SkinBounds::build(&model).unwrap().posed(&[matrix],world).expect("finite extreme bound");
    for (p,_,_,_) in model.skin(&[matrix],world) {
        assert!(p.is_finite());assert!(b.contains(p),"cancellation p={p:?}, bounds={b:?}");
    }
}

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn authored_skin_bounds_contain_vertices_and_only_reject_offscreen_geometry() {
    let root = crate::testkit::game_root();
    let mut paths:Vec<_>=std::fs::read_dir(std::path::Path::new(&root).join("NIS")).unwrap()
        .filter_map(Result::ok).map(|e|e.path()).filter(|p|p.extension().is_some_and(|e|e.eq_ignore_ascii_case("bun"))).collect();
    paths.sort();
    let (mut scenes,mut instances,mut corners,mut culled)=(0,0,0,0);
    for path in paths {
        let Ok(set)=SceneCharacters::parse(&std::fs::read(path).unwrap()) else {continue};
        if set.instances.is_empty(){continue;} scenes+=1;
        let bounds:Vec<_>=set.models.iter().map(SkinBounds::build).collect();
        for f in 0..120 {
            let origin=Mat4::from_scale_rotation_translation(Vec3::new(1.2,0.9,1.1),Quat::from_rotation_z(0.3),Vec3::new(1000.0,-2000.0,30.0));
            for (model,palette,world) in set.posed(f as f32/7.0,origin) {
                let Some(b)=bounds[model].as_ref().and_then(|b| b.posed(&palette,world)) else {continue};
                instances+=1;
                let centre=(b.min+b.max)*0.5;
                let eye=centre+Vec3::new((f as f32*0.31).sin()*15.0,-10.0,4.0);
                let target=centre+Vec3::X*((f%9) as f32-4.0)*8.0;
                let vp=Mat4::perspective_rh(0.9,16.0/9.0,0.1,1000.0)*Mat4::look_at_rh(eye,target,Vec3::Z);
                let visible=b.visible(vp);if !visible{culled+=1;}
                let mut outside=[true;4];
                for (p,_,_,_) in set.models[model].skin(&palette,world) {
                    corners+=1;assert!(b.contains(p),"scene={scenes} model={model} frame={f} p={p:?} bounds={b:?}");
                    let c=vp*p.extend(1.0);
                    for (o,d) in outside.iter_mut().zip([c.w+c.x,c.w-c.x,c.w+c.y,c.w-c.y]) {*o&=d<0.0;}
                }
                assert!(visible||outside.into_iter().any(|v|v),"rejected potentially visible geometry");
            }
        }
    }
    assert!(scenes>=18&&instances>1000&&culled>100);
    eprintln!("authored visibility: {scenes} scenes, {instances} posed instances, {corners} corners contained, {culled} conservative rejections");
}
