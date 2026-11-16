# The Enhanced levels as enh lines (src/qol/levels.rs), with the Advanced rows on Medium and Medium less one row;
# sourced by passes.sh. enh_of NAME prints a preset's lines.
LOW="enh.enhanced_paint 1;enh.enhanced_reflections 1;enh.road_reflection 1;enh.shadows 2;enh.lod_fade 1;enh.detail_distance 150;enh.wet_car 1;enh.rain_detail 1"
MED="$LOW;enh.hdr_post 1;enh.car_reflection 256;enh.wet_roads 1;enh.ssao 50;enh.dynamic_lamps 1;enh.lamp_shadows 1;enh.contact_shadows 1;enh.spray 1;enh.headlight_beams 1;enh.light_probes 1;enh.volumetric_fog 1"
HIGH="$MED;enh.hdr_post 2;enh.road_reflection 2;enh.car_reflection 1024;enh.detail_distance 200;enh.car_detail 200;enh.keep_loaded 1;enh.ssr 1;enh.lamp_shadows 2;enh.volumetric_fog 2;enh.particles 2;enh.water 1;enh.rain_detail 2;enh.wet_interaction 1;enh.enhanced_glass 1"
enh_of() {
  case $1 in
    orig) echo "" ;; low) echo "$LOW" ;; med) echo "$MED" ;; high) echo "$HIGH" ;;
    medrt) echo "$MED;enh.modern_lighting 3;enh.ssr 3" ;;
    medssaa2) echo "$MED;enh.post_aa 6" ;; medssaa4) echo "$MED;enh.post_aa 7" ;; medssaa8) echo "$MED;enh.post_aa 8" ;;
    medssaa9) echo "$MED;enh.post_aa 9" ;; medssaa16) echo "$MED;enh.post_aa 10" ;;
    medmsaa8) echo "$MED;enh.post_aa 5" ;; medmsaa4) echo "$MED;enh.post_aa 4" ;; medtaa) echo "$MED;enh.post_aa 11" ;;
    lowup3) echo "$LOW;enh.upscale 3" ;; lowup1) echo "$LOW;enh.upscale 1" ;;
    lowtaa) echo "$LOW;enh.post_aa 11" ;; hightaa) echo "$HIGH;enh.post_aa 11" ;; lowup2) echo "$LOW;enh.upscale 2" ;;
    medup2) echo "$MED;enh.upscale 2" ;; highup1) echo "$HIGH;enh.upscale 1" ;; highup2) echo "$HIGH;enh.upscale 2" ;; highup3) echo "$HIGH;enh.upscale 3" ;;
    medup1) echo "$MED;enh.upscale 1" ;; medup3) echo "$MED;enh.upscale 3" ;;
    medfsr1) echo "$MED;enh.upscale 4" ;; medfsr2) echo "$MED;enh.upscale 8" ;;
    # Medium with one row down (leave-one-out on the integrated GPU)
    med-ssao) echo "$MED;enh.ssao 0" ;; med-fog) echo "$MED;enh.volumetric_fog 0" ;; med-lamps) echo "$MED;enh.dynamic_lamps 0" ;;
    med-lsh) echo "$MED;enh.lamp_shadows 0" ;; med-hp) echo "$MED;enh.hdr_post 0" ;; med-probes) echo "$MED;enh.light_probes 0" ;;
    med-beams) echo "$MED;enh.headlight_beams 0" ;; med-wet) echo "$MED;enh.wet_roads 0" ;; med-cs) echo "$MED;enh.contact_shadows 0" ;;
    med-spray) echo "$MED;enh.spray 0" ;; med-cr) echo "$MED;enh.car_reflection 0" ;;
    *) echo "BAD" ;;
  esac
}
