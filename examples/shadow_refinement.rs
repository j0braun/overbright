//! CPU refinement benchmark and exact terrain-query comparison on shipped roads.
//! NFSU2_ROOT=... cargo run --release --example shadow_refinement
use glam::{Mat4, Quat, Vec3};
use nfsu2::{shadow::{Shadow, ShadowVertex, Ground}, ground::GroundQuery, world::World};
use std::{hint::black_box, time::Instant};
include!("fixtures/shadow-refine-reference.rs");
fn main() -> anyhow::Result<()> {
    // A developer tool: the developer cache, never the player's (docs/asset-cache.md).
    nfsu2::platform::dirs::developer_program();
    let root = nfsu2::game_root();
    let world = World::load(&root, "L4RA")?;
    let routes = nfsu2::formats::ai_routes::RouteFile::parse(&std::fs::read(root.join("TRACKS/ROUTESL4RA/RoutesFreeRoam.bin"))?);
    let box_ = (Vec3::new(-2.0,-1.0,0.0),Vec3::new(2.0,1.0,1.3));
    let wheels = [Vec3::new(1.3,0.8,0.3),Vec3::new(1.3,-0.8,0.3),Vec3::new(-1.3,-0.8,0.3),Vec3::new(-1.3,0.8,0.3)];
    let mut samples = Vec::new();
    for (i,p) in routes.records.iter().flat_map(|r| &r.points).step_by(37).take(128).enumerate() {
        let mut query = GroundQuery::default();
        let at = p.position + Vec3::Z * 0.2;
        let mut ground = |p| world.ground_for(&mut query,p).map(|(z,n,_)|(z,n));
        if let Some(shadow) = nfsu2::shadow::car_shadow(Mat4::from_rotation_translation(Quat::from_rotation_z(i as f32 * 0.37),at),box_,wheels,Some(&mut ground)) {
            samples.push((shadow,query));
        }
    }
    anyhow::ensure!(samples.len() >= 32, "need actual road samples");
    let grounded = samples.iter().filter(|(_,q)| q.pack().is_some()).count();
    anyhow::ensure!(grounded >= 32, "need loaded native collision packs, not fallback-only samples");
    println!("native collision-pack contexts: {grounded}/{}", samples.len());
    let mut total_calls = 0;
    for n in [1,2,4] { for (shadow,query) in &samples {
        let run = |optimized| {
            let mut q = query.clone(); let mut calls = Vec::new();
            let mut ground = |p: Vec3| { calls.push(p.to_array().map(f32::to_bits)); world.ground_for(&mut q,p).map(|(z,n,_)|(z,n)) };
            let out = if optimized { shadow.clone().refine(n,&mut ground) } else { reference(shadow.clone(),n,&mut ground) };
            let bits:Vec<_> = out.quads().iter().flatten().map(|v|(v.position.to_array().map(f32::to_bits),v.uv.map(f32::to_bits))).collect();
            (calls,bits,format!("{q:?}"))
        };
        let a=run(false); let b=run(true); total_calls+=a.0.len();
        anyhow::ensure!(a==b,"vertex/query/final-context mismatch n={n}");
    }}
    println!("exact: {} real road shadows × native/high/ultra; {total_calls} reference terrain queries; vertex bits, query order and final query context equal",samples.len());
    for n in [2,4] { for round in 0..6 {
        for optimized in if round%2==0 {[false,true]} else {[true,false]} {
            let now=Instant::now(); let count=20_000;
            for i in 0..count {
                let (shadow,query)=&samples[i%samples.len()];let mut q=query.clone();
                let mut ground=|p| world.ground_for(&mut q,p).map(|(z,n,_)|(z,n));
                let out=if optimized {shadow.clone().refine(n,&mut ground)} else {reference(shadow.clone(),n,&mut ground)};
                black_box(out);
            }
            if round>0 {println!("n={n} pair={round} optimized={optimized} us/shadow={:.4}",now.elapsed().as_secs_f64()*1e6/count as f64);}
        }
    }}
    Ok(())
}
