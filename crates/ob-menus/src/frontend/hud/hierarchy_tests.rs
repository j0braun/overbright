use super::*;

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn authored_hierarchy_preserves_hud_quads_across_transitions() {
    let root=crate::testkit::game_root();
    let assets=Assets::load(&root).unwrap();
    let mut indexed=Hud::default();
    let mut reference=Hud::default();
    reference.reference_hierarchy=true;
    let mut count=0;
    for cycle in 0..3 {
        for mode in [crate::race::RaceMode::FreeRoam,crate::race::RaceMode::Drag,crate::race::RaceMode::Drift,crate::race::RaceMode::StreetX,crate::race::RaceMode::Url] {
            assert!(assets.package(package_for(mode)).is_some());
            for step in 0..40 {
                let mut input=Input { race:mode, racing:step%3!=0, racers:6, career:step%2==0,
                    speed:step as f32, rpm:step as f32*180.0, clock:step as f32-3.0,
                    turbo:step%2==0, ..Default::default() };
                input.drag=(mode==crate::race::RaceMode::Drag && step%2==0).then(DragHud::default);
                input.options.map=step%4!=0;
                input.options.gauges=step%5!=0;
                input.options.leaderboard=step%3!=0;
                let now=count as f32*16.0;count+=1;
                let a=indexed.quads(&assets,&input,now);let b=reference.quads(&assets,&input,now);
                assert_eq!(indexed.state.hidden,reference.state.hidden,"cycle={cycle} mode={mode:?} step={step}");
                assert_eq!(indexed.state.shown,reference.state.shown);
                assert_eq!(a.len(),b.len());
                for (a,b) in a.iter().zip(&b) {
                    assert_eq!(a.texture,b.texture);
                    assert_eq!(format!("{:?}",a.place),format!("{:?}",b.place));
                    let bits=|q:&Quad| q.rect.iter().chain(&q.uv).flat_map(|v|v.to_array()).chain(q.color).chain([q.z,q.angle]).map(f32::to_bits).collect::<Vec<_>>();
                    assert_eq!(bits(a),bits(b),"exact quad fields and order");
                }
            }
        }
    }
    eprintln!("exact HUD hierarchy: {count} frames, all five mode/package selections and transitions");
}

#[test]
#[ignore="controlled CPU benchmark; original assets required"]
fn hud_hierarchy_paired_cpu() {
    let assets=Assets::load(&crate::testkit::game_root()).unwrap();
    let input=Input { racing:true, racers:4, career:true, ..Default::default() };
    for pair in 0..5 {
        for reference in if pair%2==0 {[true,false]} else {[false,true]} {
            let mut hud=Hud::default();hud.reference_hierarchy=reference;
            for frame in 0..100 { std::hint::black_box(hud.quads(&assets,&input,frame as f32*16.0)); }
            let at=std::time::Instant::now();
            for frame in 100..2100 {std::hint::black_box(hud.quads(&assets,&input,frame as f32*16.0));}
            eprintln!("pair={pair} reference={reference} us/frame={:.3}",at.elapsed().as_secs_f64()*1e6/2000.0);
        }
    }
}
