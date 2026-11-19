//! The rule registry (docs/modding/asset-format/conventions.md §11): one entry per rule id of the specification, with
//! the page that defines it and its text. Every refusal of this crate names one ([`crate::Refusal`]), and the
//! pages' rule tables are generated from this list ([`crate::docs`]), so a rule id exists in the code exactly when
//! it exists in the docs.

macro_rules! rules {
    ($($var:ident $id:literal $page:literal $text:literal;)*) => {
        /// A rule of the specification. Rule ids are stable: a changed rule gets a new id (versioning.md P.7).
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub enum Rule {
            $(
                #[doc = $text]
                $var,
            )*
        }

        impl Rule {
            /// Every rule, in the order of the pages' tables.
            pub const ALL: &'static [Rule] = &[$(Rule::$var),*];

            /// The rule id (`CONV.3`).
            pub const fn id(self) -> &'static str {
                match self { $(Rule::$var => $id,)* }
            }

            /// The page that defines it (`conventions.md`).
            pub const fn page(self) -> &'static str {
                match self { $(Rule::$var => $page,)* }
            }

            /// The rule's text as its page's table shows it (Markdown).
            pub const fn text(self) -> &'static str {
                match self { $(Rule::$var => $text,)* }
            }
        }
    };
}

impl Rule {
    /// The rule with this id.
    pub fn from_id(id: &str) -> Option<Rule> {
        Rule::ALL.iter().copied().find(|r| r.id() == id)
    }
}

impl std::fmt::Display for Rule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.id())
    }
}

/// Rule ids a page reserves for a later rule: never used for another one.
pub const RESERVED: &[&str] = &["WLD.16", "WLD.17"];

rules! {
    Ani1 "ANI.1" "animation.md" r#"An `.eagl` file that is not an ELF32 little-endian relocatable object, lacks the `__AnimationBank` (banks) or `__Skeleton:::` (skeletons) symbol, or whose digest differs from `digest`."#;
    Ani2 "ANI.2" "animation.md" r#"A bank index that differs from its object: channel count, names or order; a skeleton name or bone count."#;
    Ani3 "ANI.3" "animation.md" r#"A `keys` channel whose `values` count is not a positive multiple of `width`, whose `width` is 0 or over 64, whose `slots` length differs from `width`, or whose `frames` is 0."#;
    Ani4 "ANI.4" "animation.md" r#"A bank over the decode limit (4,194,304 values, `crates/ob-formats/src/formats/reader.rs:94`), or a channel name outside §2.3."#;
    Ani5 "ANI.5" "animation.md" r#"Camera tracks over the limits: more groups than the kind allows (3,000 / 256 / 100), more than 255 entries in a track, a name over 13 characters; a group a pack adds with a `key` the merged file already has."#;
    Ani6 "ANI.6" "animation.md" r#"A scene whose `name` differs from its unit's `<NAME>`, is over 15 characters or outside `[A-Za-z0-9_]`, or whose `hash` is not `bin_hash(name)`; a description over 200 characters or outside printable ASCII."#;
    Ani7 "ANI.7" "animation.md" r#"An event with more than 16 arguments, a token outside the three forms (a string token in an integer or float form included), or a float token that is not finite."#;
    Ani8 "ANI.8" "animation.md" r#"An override of a channel in an object with `decodes = false`."#;
    Ani9 "ANI.9" "animation.md" r#"An authored skeleton with a parent not below its own index, more than 255 bones, or no root."#;
    Ani10 "ANI.10" "animation.md" r#"Pivots that are not exactly 21, or a `car_hash` that is not `bin_hash` of the unit's car name."#;
    Aud1 "AUD.1" "audio.md" r#"A bank image that does not parse: no `ABKC`, `file_size` ≠ the array's count, a table offset outside the image."#;
    Aud2 "AUD.2" "audio.md" r#"A bank index that differs from its image (module count, events, any `sound` field)."#;
    Aud3 "AUD.3" "audio.md" r#"A FLAC source outside the profile (§3.8): wrong channels for its use, bits per sample, rate, missing audio checksum or sample count, Ogg FLAC, over the limits."#;
    Aud4 "AUD.4" "audio.md" r#"A replacement source that is missing, not FLAC, or whose digest differs."#;
    Aud5 "AUD.5" "audio.md" r#"An engine recording whose fields disagree with its `gin` file, whose tables are not ordered as §3.3 says, or whose `min_rpm` ≥ `max_rpm`."#;
    Aud6 "AUD.6" "audio.md" r#"A speech line range outside `lines`, records out of order, keys of uneven length, `ring` longer than `history`, or a line that is not one `ea_stream`."#;
    Aud7 "AUD.7" "audio.md" r#"A music map with a stream range outside `streams_data` or not 128-aligned, a node `next` or event `start_node` out of range, a `stream_id` past the stream table."#;
    Aud8 "AUD.8" "audio.md" r#"A mix map whose 3D variant count or channel word count differs from its id word, or with more than 32 sections."#;
    Aud9 "AUD.9" "audio.md" r#"A reverb preset with a module kind above 9, `p4`/`p6` on a 4-byte kind (or missing on an 8-byte one), a link to a module that does not exist, or not exactly 6 configurations."#;
    Aud10 "AUD.10" "audio.md" r#"An EA stream that breaks §3.1.3: no `SCHl` first, a chunk size under 8 or past the range, an unknown codec, block frame counts that do not sum to the header's."#;
    Aud11 "AUD.11" "audio.md" r#"A `sound` override of a slot the base marks `present = false` (the engine never starts such a slot; overrides of items that do not exist at all are `MRG.1`)."#;
    Crr1 "CRR.1" "career.md" r#"Two items of one table with the same key; a `hash` field in a mod pack, or one equal to `bin_hash(name)` (it is written only when they differ)."#;
    Crr2 "CRR.2" "career.md" r#"A string longer than its record field (§3.5), or with a character outside printable ASCII (the records hold single-byte text)."#;
    Crr3 "CRR.3" "career.md" r#"A merge of a field not marked **M** (this kind's case of MRG.3; reported as CRR.3 with the field)."#;
    Crr4 "CRR.4" "career.md" r#"`races` empty or with more than 4 entries; `opponents` over 5; `goals` over 8; `sponsors` over 5."#;
    Crr5 "CRR.5" "career.md" r#"An event with both `goals` and `opponents`."#;
    Crr6 "CRR.6" "career.md" r#"A merge of `races` that changes a track or variant or the number of races; a merge of `opponents` that changes their number."#;
    Crr7 "CRR.7" "career.md" r#"Two track records with the same number that differ after merging."#;
    Crr8 "CRR.8" "career.md" r#"`track-maps`: a non-finite value, `zoom ≤ 0`, `window_size < 0`, or a number with no track record."#;
    Crr9 "CRR.9" "career.md" r#"A legacy `map-overrides.tsv` with an unknown field, a duplicate row or a malformed line (the HUD mod's pack is not mounted)."#;
    Crr10 "CRR.10" "career.md" r#"`magazines`: a list longer than its slots."#;
    Crr11 "CRR.11" "career.md" r#"(warning) A track whose `region` has no `world/<REGION>/` unit in the stack: its races cannot run and it is not offered."#;
    Crr12 "CRR.12" "career.md" r#"Two files of one pack set the same field of one item (§2)."#;
    Crs1 "CRS.1" "cars.md" r#"`index.toml` missing in a base set, a name in it without `types/<NAME>.toml`, or a type file not in it."#;
    Crs2 "CRS.2" "cars.md" r#"A text field longer than its record field allows (N − 1 bytes) or with characters outside printable ASCII; a `raw_text` value whose bytes up to the first NUL differ from the text field, or `raw_text` in a mod pack."#;
    Crs3 "CRS.3" "cars.md" r#"A merged car type that cannot run: NaN or infinite values, mass, wheel radius or rpm span not above 0, `forward_gears` outside 1–6, a camera count above 6 (packs and community sets)."#;
    Crs4 "CRS.4" "cars.md" r#"`ai_preset.parts` not ascending, repeated, or above 71."#;
    Crs5 "CRS.5" "cars.md" r#"A preset whose `car` names no car type, or a slot key above 0xA9."#;
    Crs6 "CRS.6" "cars.md" r#"Part database: an index out of range (group, name, list, model, string `at` not increasing), a list running past `lists`, group hashes not unique, (group, hash) not unique, `header` counts unlike the pools, or not exactly one of `string` and `string_pool` in a base set."#;
    Crs7 "CRS.7" "cars.md" r#"A mod pack sets a field this page reserves to base sets (the part pools and record fields of §3.4; any field of `index.toml` or `unit.toml`)."#;
    Crs8 "CRS.8" "cars.md" r#"Cost table: `number` not equal to the position + 1 in a base set, or repeated."#;
    Crs9 "CRS.9" "cars.md" r#"Performance: a repeated package (category, level), performance part id or tuning unlock (part, param); an id above 0x43; `param` above 24; `replaces` / `requires` naming no performance part."#;
    Crs10 "CRS.10" "cars.md" r#"Car unit: `name` unlike its folder, `role = "car"` without a resolvable `car`, a car unit without `GEOMETRY.BIN` data or without any `TEXTURES.BIN` entry, a `data` file without `paths` once its kind has graduated, `paths` on an `empty` file."#;
    Crs11 "CRS.11" "cars.md" r#"Import (warning): `GLOBALB.BUN` and `GlobalB.lzc` differ in a chunk of this page; the edition is `custom`."#;
    Crs12 "CRS.12" "cars.md" r#"Import (warning): a derived field differs from its derivation, a text field needs `raw_text`, or a record holds fill the encoder does not write (unused package slots, cost pads); the field is written explicitly, or those bytes become patches (a shadow beyond RES.6's limit)."#;
    Crs13 "CRS.13" "cars.md" r#"Import (warning): the bytes after the last car record name a part group, so the original's past-the-end read would find a car; the edition is `custom`."#;
    Col1 "COL.1" "collision.md" r#"A pack file whose `section` differs from its file name or names no stream section of the region; two packs for one stream section."#;
    Col2 "COL.2" "collision.md" r#"`cells` does not have `width × height` entries."#;
    Col3 "COL.3" "collision.md" r#"A cell's range outside `refs`, or a reference's face outside `faces`."#;
    Col4 "COL.4" "collision.md" r#"A cell lists a floor after a wall or a 0x08 face."#;
    Col5 "COL.5" "collision.md" r#"A face whose `cache_slot` is not 0xFFFF."#;
    Col6 "COL.6" "collision.md" r#"A face whose `group` has no barrier group."#;
    Col7 "COL.7" "collision.md" r#"A mod pack holding a file of this page."#;
    Col8 "COL.8" "collision.md" r#"More than 256 barrier groups, or two with the same name."#;
    Col9 "COL.9" "collision.md" r#"A volume whose `support`, `first` or `count` length differs from its vertices, or whose links run outside `links` or name a missing vertex."#;
    Conv1 "CONV.1" "conventions.md" r#"Not UTF-8, has a BOM or a NUL."#;
    Conv2 "CONV.2" "conventions.md" r#"TOML syntax error (with the parser's line and column)."#;
    Conv3 "CONV.3" "conventions.md" r#"Missing `schema`, or a kind this reader does not know (versions: `VER.1`, `VER.2`)."#;
    Conv4 "CONV.4" "conventions.md" r#"Unknown key outside `[extras]` / `[ext.*]`."#;
    Conv5 "CONV.5" "conventions.md" r#"Missing required field."#;
    Conv6 "CONV.6" "conventions.md" r#"Value of the wrong type, out of range, or not in the enum."#;
    Conv7 "CONV.7" "conventions.md" r#"Float that is NaN or infinite where not allowed, or a malformed `bits:` escape."#;
    Conv8 "CONV.8" "conventions.md" r#"Bad name, hash, digest or reference syntax."#;
    Conv9 "CONV.9" "conventions.md" r#"Reference that resolves to nothing in the layer stack."#;
    Conv10 "CONV.10" "conventions.md" r#"Array descriptor out of range, misaligned, overlapping, or of an unknown layout."#;
    Conv11 "CONV.11" "conventions.md" r#"Non-zero padding in a binary struct."#;
    Conv12 "CONV.12" "conventions.md" r#"A limit exceeded."#;
    Conv13 "CONV.13" "conventions.md" r#"Two logical paths differing only in case; a path that breaks 6.2; two keys of a file-name kind (§6.3) differing only in case within one layer stack."#;
    Conv14 "CONV.14" "conventions.md" r#"A required extension this reader does not know."#;
    Conv15 "CONV.15" "conventions.md" r#"A new name whose hash collides with another name of the same kind in the stack."#;
    Conv16 "CONV.16" "conventions.md" r#"An `[ext.X]` table not declared in `extensions_used`, or an always-required `OB_` extension declared only as used."#;
    Tbl1 "TBL.1" "engine-tables.md" r#"Unknown table name, or `table` unlike the file name."#;
    Tbl2 "TBL.2" "engine-tables.md" r#"The table is not overridable: its class (`fixed`, `formula`, `decoder`, `derived`) is named."#;
    Tbl3 "TBL.3" "engine-tables.md" r#"Shape mismatch: a path that does not exist, an array of the wrong length, a value of the wrong type, an unknown struct field."#;
    Tbl4 "TBL.4" "engine-tables.md" r#"A value out of its type's range, NaN or infinity where the built-in value is finite, a string too long or not printable ASCII."#;
    Tbl5 "TBL.5" "engine-tables.md" r#"A `guard` unlike the running program's built-in value at its path."#;
    Tbl6 "TBL.6" "engine-tables.md" r#"Overrides installed after the tables were first used, or installed twice (a program error)."#;
    Tbl7 "TBL.7" "engine-tables.md" r#"`tables/` in an imported base set (one with `import.toml`)."#;
    Tbl8 "TBL.8" "engine-tables.md" r#"A value that indexes another table or names an item is out of range or resolves to nothing."#;
    Tbl9 "TBL.9" "engine-tables.md" r#"Two entries of one file address the same value, or one inside another."#;
    Tbl10 "TBL.10" "engine-tables.md" r#"`op = "replace"` without `value`, a `merge` without `set`, or both `value` and `set`."#;
    Imp1 "IMP.1" "import.md" r#"The folder has no content folders (not a game folder), or a required file is missing (named)."#;
    Imp2 "IMP.2" "import.md" r#"Not enough free space (needed, free, where)."#;
    Imp3 "IMP.3" "import.md" r#"A unit's read-back model differs from the original's (unit, item, both hashes): an Overbright bug; nothing is written."#;
    Imp4 "IMP.4" "import.md" r#"A residual map's rebuild digest differs: an Overbright bug; nothing is written."#;
    Imp5 "IMP.5" "import.md" r#"The required-items profile is incomplete (a `custom` install missing files)."#;
    Imp6 "IMP.6" "import.md" r#"The staging folder belongs to another edition or importer version than this run; it is discarded and the import starts over."#;
    Imp7 "IMP.7" "import.md" r#"A release build found a `linked` base set (it offers to make it independent), or a linked set's referenced file is missing or changed."#;
    Ord1 "ORD.1" "layering.md" r#"`order.txt` without its first line, or of an unknown version."#;
    Ord2 "ORD.2" "layering.md" r#"A dependency missing, off, or above its dependent; or a pack that merges or replaces an item another pack added without listing that pack in `[dependencies]`."#;
    Ord3 "ORD.3" "layering.md" r#"Two packs listed in each other's `[conflicts]` are both on."#;
    Ord4 "ORD.4" "layering.md" r#"A dependency of this pack was refused (named), so this pack is refused too."#;
    Mrg1 "MRG.1" "layering.md" r#"`merge` or `replace` of an item that no lower layer has."#;
    Mrg2 "MRG.2" "layering.md" r#"`add` of an item a lower layer already has (use `merge`)."#;
    Mrg3 "MRG.3" "layering.md" r#"An operation the kind does not allow (e.g. `add` of a car before add-on cars exist)."#;
    Mrg4 "MRG.4" "layering.md" r#"A keyed-list element with an unknown key and no `add`."#;
    Mrg5 "MRG.5" "layering.md" r#"In a pack not marked `from-game-files` / `local-only`: a file, array or byte range equal to an original file, a residual node or a base-set file or payload, or a file the journal records as derived from base-set content."#;
    Mrg6 "MRG.6" "layering.md" r#"A mod pack with `residual/` or `import.toml`."#;
    Lay1 "LAY.1" "layout.md" r#"A bundle breaks the ZIP profile (§4): 64-bit ZIP extensions, encryption, data descriptor, unknown method, misaligned stored entry, bad name, duplicate, overlap."#;
    Lay2 "LAY.2" "layout.md" r#"Two bundles of one pack hold the same logical path, or a bundle holds files outside its subtree (§5)."#;
    Lay3 "LAY.3" "layout.md" r#"A base-set bundle's digest does not match `import.toml` / `[bundles]`, or a listed bundle is missing."#;
    Lay4 "LAY.4" "layout.md" r#"A symbolic link or a name outside conventions §6.2 in a pack folder."#;
    Lay5 "LAY.5" "layout.md" r#"`active.txt` names a base set that does not exist or is damaged (the launcher asks; the game refuses to start)."#;
    Man1 "MAN.1" "manifest.md" r#"`format` higher than this reader knows, or not an integer."#;
    Man2 "MAN.2" "manifest.md" r#"`id` invalid, or not matching the `.obpk` file name."#;
    Man3 "MAN.3" "manifest.md" r#"This Overbright version is outside `overbright`."#;
    Man4 "MAN.4" "manifest.md" r#"`share = "public"` with an origin that forces `local-only`; `public` without `licence`."#;
    Man5 "MAN.5" "manifest.md" r#"`[base]` in a mod pack, or missing in a base set."#;
    Man6 "MAN.6" "manifest.md" r#"A base set lacks an item of its profile (the first ten missing keys are listed)."#;
    Man7 "MAN.7" "manifest.md" r#"A dependency, optional or conflict entry names the pack itself, or a version requirement does not parse."#;
    Man8 "MAN.8" "manifest.md" r#"`pack.toml` larger than 64 KiB (as today, `src/obpack/manifest.rs` `MAX_BYTES`)."#;
    Man9 "MAN.9" "manifest.md" r#"`extensions_required` not a subset of `extensions_used`."#;
    Msh1 "MSH.1" "meshes.md" r#"A mesh file outside the profile (§3.1): not glTF 2.0, more or fewer than one buffer or scene, a data URI, a `.bin` of another stem, `OB_` names in `extensionsRequired`, no `OB_solid_list.schema` or one of another kind, an `OB_` extension in `extensionsUsed` without its `version` in the root `extensions` (or a root entry for one not used), no `OB_Z_UP` root or another rotation, a property the profile excludes."#;
    Msh2 "MSH.2" "meshes.md" r#"A child of `OB_Z_UP` without `OB_solid`, or a child of a solid node without `OB_marker`."#;
    Msh3 "MSH.3" "meshes.md" r#"Vertex data off its layout: wrong stride, attribute offset, component type or `normalized`; a view not 16-aligned; an attribute missing or extra for the layout."#;
    Msh4 "MSH.4" "meshes.md" r#"An index ≥ the vertex count or equal to 65535; an index accessor outside the index view; a count that is 0 or not a multiple of 3 on a primitive."#;
    Msh5 "MSH.5" "meshes.md" r#"More than 65,535 vertices in a solid; a vertex count that is not the same for all of a solid's attributes."#;
    Msh6 "MSH.6" "meshes.md" r#"Draw ranges and primitives disagree (number, `first_index`/`index_count` vs. the accessor); a `texture_slot` past `textures`; a `light_material` other than 0xFF past `light_materials`; over 255 ranges, textures or light materials."#;
    Msh7 "MSH.7" "meshes.md" r#"`POSITION` `min`/`max` that differ from the data."#;
    Msh8 "MSH.8" "meshes.md" r#"A hash that is not `bin_hash(node.name)` without `name_truncated`; `name_truncated` with a name not 27 characters long; two solids of one list with the same hash."#;
    Msh9 "MSH.9" "meshes.md" r#"A marker node whose matrix is missing, not affine (w column ≠ 0, 0, 0, 1) or not decomposable."#;
    Msh10 "MSH.10" "meshes.md" r#"An `extra_chunks` entry with both or neither of `bufferView` and `bytes`, `bytes` over 64, or an id with bit 31 set."#;
    Msh11 "MSH.11" "meshes.md" r#"A buffer smaller than its `byteLength`, overlapping views, or a view outside the buffer."#;
    Msh12 "MSH.12" "meshes.md" r#"In a pack: a solid node without `op`; `replace` without `target` or with another hash; `add` of a name the stack has (also `MRG.2`)."#;
    Msh13 "MSH.13" "meshes.md" r#"A mesh-patch guard or `from` value that does not match the stack below; a patched index past the solid's vertices, ranges, markers or triangles."#;
    Msh14 "MSH.14" "meshes.md" r#"A layout the unit's consumer cannot draw (`skinned60` outside scenes, `world24` on a car part that needs normals); missing UVs."#;
    Msh15 "MSH.15" "meshes.md" r#"An export view (`extras.ob_view`) used as a pack's mesh file."#;
    Msh16 "MSH.16" "meshes.md" r#"A mesh-patch target, or a `replace` node's `target`, outside the unit of the file that holds it."#;
    Mov1 "MOV.1" "movies.md" r#"A `video.ivf` header that is not `DKIF`, version 0, header length 32, a FourCC outside `VP60`/`VP61`/`VP62` for codec `vp6`, or whose width, height, time base or frame count differs from `movie.toml`."#;
    Mov2 "MOV.2" "movies.md" r#"A frame the engine's VP6 decoder refuses (version > 7, profile ≠ 0, interlaced), or a first frame that is not a key frame."#;
    Mov3 "MOV.3" "movies.md" r#"Frame timestamps that are not 0, 1, 2, …; a frame size of 0; a frame running past the file; a largest frame other than `largest_frame`."#;
    Mov4 "MOV.4" "movies.md" r#"An audio stream that is not one `ea_stream` with a `GSTR` or `PT` header, or whose codec, channels, rate or frame count differ from `movie.toml`."#;
    Mov5 "MOV.5" "movies.md" r#"A `chunk_order` array whose length differs from the stream's chunk count, that decreases, or that exceeds `frames`."#;
    Mov6 "MOV.6" "movies.md" r#"A movie-table name that is empty, longer than 30 characters, not ASCII, or a volume above 127."#;
    Mov7 "MOV.7" "movies.md" r#"A `video.codec` or audio codec this reader does not support (values reserved for §9)."#;
    Mov8 "MOV.8" "movies.md" r#"Two `movie-entry` names that differ only in case in one layer stack (two such `movie` stems: `CONV.13`); a movie folder name that differs from `name`."#;
    Res1 "RES.1" "residual.md" r#"Nodes do not cover the content exactly once, are out of order, or a node's range runs past `content_size`."#;
    Res2 "RES.2" "residual.md" r#"A `values` node without kept bytes; a `kept` node whose descriptor is out of range."#;
    Res3 "RES.3" "residual.md" r#"A rebuilt file's size or digest differs from the map."#;
    Res4 "RES.4" "residual.md" r#"A residual file in a mod pack, or a map naming a file outside the game's content folders."#;
    Res5 "RES.5" "residual.md" r#"A `copy` node whose source range lies outside its file, or copy nodes that refer to each other in a cycle."#;
    Res6 "RES.6" "residual.md" r#"Patches out of order, outside the node, or a `patched` node whose patches exceed 1/16 of its size (then it is a `values` node with a shadow)."#;
    Res7 "RES.7" "residual.md" r#"An `external` node in an owned base set, or in any mod pack."#;
    Rte1 "RTE.1" "roads-ai.md" r#"The points array does not hold exactly the sum of the records' `point_count`s, or an array descriptor's layout does not match its field."#;
    Rte2 "RTE.2" "roads-ai.md" r#"Route record ids not unique, not ascending, or ≥ 2048."#;
    Rte3 "RTE.3" "roads-ai.md" r#"A `.bin` array whose bytes break a checked constant (junction prefix, marker constants) when rebuilt."#;
    Rte4 "RTE.4" "roads-ai.md" r#"Triggers not sorted by `kind`, or a `kind` above 20."#;
    Rte5 "RTE.5" "roads-ai.md" r#"A kind-16 trigger without `traffic`, or `traffic` on another kind; `name`/`gps` together with `traffic`."#;
    Rte6 "RTE.6" "roads-ai.md" r#"A barrier with more than 4 groups; a checkpoint list with more than 33 points or a track number twice in one table."#;
    Rte7 "RTE.7" "roads-ai.md" r#"A `nameref` that starts with `0x` but is not a `hash32` (8 upper-case hex digits), or a name that is not a valid game name ([conventions.md](conventions.md) §6.3)."#;
    Rte8 "RTE.8" "roads-ai.md" r#"A race `id` that breaks `[a-z0-9_-]{1,40}` or differs from its file name."#;
    Rte9 "RTE.9" "roads-ai.md" r#"`laps` outside 1–20, or not 1 for a sprint or drift race."#;
    Rte10 "RTE.10" "roads-ai.md" r#"More than 64 checkpoints or more than 8 grid slots."#;
    Rte11 "RTE.11" "roads-ai.md" r#"A race coordinate or heading not finite or beyond ±100,000."#;
    Rte12 "RTE.12" "roads-ai.md" r#"A race `name` over 200 characters or with a control character."#;
    Rte13 "RTE.13" "roads-ai.md" r#"(warning) A race that cannot be raced: fewer than 2 checkpoints, a track number with no road network, a checkpoint leg with no way through the open network, or no career event of its mode on the base track. It is kept and shown with the reason, but not offered."#;
    Rte14 "RTE.14" "roads-ai.md" r#"A `name` / `region` / `track` field that does not match the file's path."#;
    Txt1 "TXT.1" "text.md" r#"A language key that breaks `[a-z][a-z0-9_]{0,31}`, or a `text/<language>/` unit without `language.toml`."#;
    Txt2 "TXT.2" "text.md" r#"Two entries with one `hash` in a `strings.toml`."#;
    Txt3 "TXT.3" "text.md" r#"An entry with both `text` and `bytes`, or `bytes` holding a zero byte."#;
    Txt4 "TXT.4" "text.md" r#"An added string without `label`, or with `bin_hash(label)` ≠ `hash`; an imported `label` matching neither `bin_hash(label)` nor that of its upper-case form."#;
    Txt5 "TXT.5" "text.md" r#"A text holding U+0000, or longer than its limit."#;
    Txt6 "TXT.6" "text.md" r#"A `charmap` that is not exactly one `charmap_region` (6,148 bytes)."#;
    Txt7 "TXT.7" "text.md" r#"A `lines` element holding CR or LF; a licence text with a character its `encoding` cannot write."#;
    Txt8 "TXT.8" "text.md" r#"Save texts: more than 256 entries."#;
    Txt9 "TXT.9" "text.md" r#"A subtitle cue with time 0xFFFF (reserved for the end record)."#;
    Txt10 "TXT.10" "text.md" r#"A pack changing a base language's `role` or `charmap`, or adding a language with `role` other than `player`."#;
    Txt11 "TXT.11" "text.md" r#"(import) A file whose rebuild differs from the original: kept whole in the residual and reported."#;
    Txt12 "TXT.12" "text.md" r#"A language of role `player` without `strings.toml` (after merging the stack), or one of role `save_text` with a `strings.toml`, `charmap` or `fonts`."#;
    Tex1 "TEX.1" "textures.md" r#"`index.toml`: an entry naming a missing file, a duplicate `id`, `section` missing where §3.1 requires it or present where it refuses it, a `set` entry in a wildcard scope's index."#;
    Tex2 "TEX.2" "textures.md" r#"A set or `.anims.toml` file in `textures/` that the index does not list."#;
    Tex3 "TEX.3" "textures.md" r#"An index entry in a mod pack without `op = "add"`, or a mod pack holding a file of a base set's set."#;
    Tex4 "TEX.4" "textures.md" r#"Two records with one hash in one set; two animation records with one base hash in one file."#;
    Tex5 "TEX.5" "textures.md" r#"An `image` that does not exist, lies outside the file's folder, or is named by two records."#;
    Tex6 "TEX.6" "textures.md" r#"A KTX 2.0 file outside the profile (§3.3): identifier, vkFormat, `typeSize`, depth, layers, faces, `levelCount` 0 or above the full chain, supercompression other than 0 or 2, a level's length, a data format descriptor that does not match the vkFormat, `KTXorientation` other than `rd`, a level that does not inflate to its length."#;
    Tex7 "TEX.7" "textures.md" r#"An image whose level-0 size differs from `width_px` × `height_px`."#;
    Tex8 "TEX.8" "textures.md" r#"`R8_UINT` without a 1,024-byte `OBpalette`, or `OBpalette` on another format."#;
    Tex9 "TEX.9" "textures.md" r#"`full_name` whose `bin_hash` is not `hash`, or whose first 23 characters are not `name`; a `name` longer than 23 characters or not a `name` (conventions §3.2)."#;
    Tex10 "TEX.10" "textures.md" r#"A `tpk` table the image contradicts: `tpk.compression` not P8 on a non-`R8_UINT` image (or the reverse), a side that is not a power of two, a `tpk` table on a PNG or DDS image."#;
    Tex11 "TEX.11" "textures.md" r#"A PNG or DDS image that does not decode, exceeds 4,096 per side (overrides) or 8,192 (sets) or 128 MiB decoded, or (BC) is not whole 4 × 4 blocks."#;
    Tex12 "TEX.12" "textures.md" r#"An image type that is not PNG, DDS or KTX 2.0, or a DDS array or cube map."#;
    Tex13 "TEX.13" "textures.md" r#"`overrides.toml` or `images/` outside a view scope (§3.5)."#;
    Tex14 "TEX.14" "textures.md" r#"Two entries for one (scope, hash, renderer) in one pack, or two images for one key in one scope."#;
    Tex15 "TEX.15" "textures.md" r#"An `add` or `replace` entry without an image."#;
    Tex16 "TEX.16" "textures.md" r#"A texture a mod pack adds (an `add` entry or a record of an added set) whose hash already resolves below in at least one view of its scope (§3.5)."#;
    Tex17 "TEX.17" "textures.md" r#"An added or overridden hash equal to a name the engine registers for a run-time texture."#;
    Tex18 "TEX.18" "textures.md" r#"`width_px` / `height_px` on a `merge` or `replace` entry."#;
    Tex19 "TEX.19" "textures.md" r#"A scroll value outside `i16`, or an animation `add` without `fps` and `frames`."#;
    Tex20 "TEX.20" "textures.md" r#"A view whose search path holds two different textures under one hash (import check; an error for known editions, a warning for `custom`)."#;
    Ui1 "UI.1" "ui.md" r#"A package `name` that breaks its pattern, or a package file not named after it."#;
    Ui2 "UI.2" "ui.md" r#"Two objects with one `id` in a package; two scripts with one `name_hash` on an object; two nodes with one `message` in a response list."#;
    Ui3 "UI.3" "ui.md" r#"A `parent` that names no earlier `group` object of the package."#;
    Ui4 "UI.4" "ui.md" r#"A `resource` index outside the resource list."#;
    Ui5 "UI.5" "ui.md" r#"A `state` of the wrong shape: `corner_colors_bgra` without `uv`, or two objects of one kind with different state sizes."#;
    Ui6 "UI.6" "ui.md" r#"`text` on an object that is not a string, or a string object without it."#;
    Ui7 "UI.7" "ui.md" r#"A track whose `delta` lengths differ from `base`, whose values have the wrong type for its `param`, whose `target_word` plus value count passes the state's words, or whose `length_ms` is ≥ 2²⁴."#;
    Ui8 "UI.8" "ui.md" r#"A `chain` that names no script of the same object."#;
    Ui9 "UI.9" "ui.md" r#"A two-character tag that would encode to more than 65,535 bytes."#;
    Ui10 "UI.10" "ui.md" r#"A `name` or `label` whose hash differs from the stored hash (`fe_hash` for objects and scripts; `bin_hash` of the label or of its upper-case form for labels)."#;
    Ui11 "UI.11" "ui.md" r#"A font with glyph codes not strictly ascending, more than 255 pairs on a glyph, `flags` without bit 0x40000, name or texture blocks over 256 bytes with their NULs, or a `tail` over 4,096 bytes."#;
    Ui12 "UI.12" "ui.md" r#"A font `texture` that resolves to no texture in the layer stack."#;
    Ui13 "UI.13" "ui.md" r#"One font name twice in one unit, or a language `fonts` entry that neither the language unit nor `frontend/ui/fonts/` holds."#;
    Ui14 "UI.14" "ui.md" r#"(import) A package or font chunk the model cannot represent exactly: kept whole in the residual and reported."#;
    Ui15 "UI.15" "ui.md" r#"(import) Two package chunks of one name whose unpacked bytes differ: the first in the original load order is decoded, the other kept and reported."#;
    Ui16 "UI.16" "ui.md" r#"A `state` colour or rotation, or a track's `base` or `delta`, in another form than the original's words (§3.1, "Exception to conventions §5.3 and §5.4"): a `color` string, a matrix, Euler angles."#;
    Ver1 "VER.1" "versioning.md" r#"A file's schema minor is higher than this reader knows (the message names the release that added it, when known)."#;
    Ver2 "VER.2" "versioning.md" r#"A file's schema major is unknown (neither readable nor migratable)."#;
    Ver3 "VER.3" "versioning.md" r#"A migration failed (the file is refused with the migration's own rule id)."#;
    Ver4 "VER.4" "versioning.md" r#"A base set's import epoch is newer than this Overbright (an older release started on a newer base set): the launcher refuses and says which release made it."#;
    Ver5 "VER.5" "versioning.md" r#"A rename table, applied as a whole (all renames at once, so two names may swap), leaves two items with one key."#;
    Ver6 "VER.6" "versioning.md" r#"A file uses a field or enum value introduced after the minor version it declares (under-declared `schema`)."#;
    Wld1 "WLD.1" "world.md" r#"A file of this page in the wrong place: `region` not the unit's name; a stream-section file whose `number` is not in `region.toml` or not its file name."#;
    Wld2 "WLD.2" "world.md" r#"Two stream sections with the same number, or a stream section's name that is empty or longer than 7 characters."#;
    Wld3 "WLD.3" "world.md" r#"A scenery section number used twice in a region, or more than two scenery sections in a stream section."#;
    Wld4 "WLD.4" "world.md" r#"A list given in both forms or in neither where one is required; an external array of the wrong layout for its field; at import, a *derived* row (§3.1: a count or a copy of the stream section's number) that does not match."#;
    Wld5 "WLD.5" "world.md" r#"An instance's `info` beyond its scenery section's infos."#;
    Wld6 "WLD.6" "world.md" r#"A `const` row (§3.1) with another value: in an external array, or at import (other than a version word, whose chunk is kept in the residual instead); alignment fill other than `0x11` at import. At import it means the edition differs from this specification."#;
    Wld7 "WLD.7" "world.md" r#"A rotation or position that is not finite."#;
    Wld8 "WLD.8" "world.md" r#"A visible list longer than 72, or an outline with fewer than 3 points."#;
    Wld9 "WLD.9" "world.md" r#"A scenery link (or a prop instance's `link`) naming no original placement."#;
    Wld10 "WLD.10" "world.md" r#"A mod pack holding a file of this page other than `placements.toml`, or a base set holding `placements.toml`."#;
    Wld11 "WLD.11" "world.md" r#"A guard that does not match the base set's placement (solid differs or position more than 0.05 m off). Refuses the whole pack (§4.3)."#;
    Wld12 "WLD.12" "world.md" r#"In `placements.toml`: a rotation whose rows are not unit within 0.01 or whose \|det\| < 0.9; a coordinate beyond ±100,000 m."#;
    Wld13 "WLD.13" "world.md" r#"In `placements.toml`: two entries with the same key; an entry with neither `key` nor `id`; `merge` of an original without `guard`; `add` without `id`, `scenery`, `solid` or `position`, or with `key`; an invalid `id`."#;
    Wld14 "WLD.14" "world.md" r#"An addition's `scenery` names no scenery section of the region. Refuses the whole pack (§4.3)."#;
    Wld15 "WLD.15" "world.md" r#"Warning, legacy format-1 map mods only: an edit that resolves to no placement is skipped and counted in the log; the pack stays mounted (§4.4)."#;
    Wld18 "WLD.18" "world.md" r#"A parameter map without rows, or a row whose length differs from its `types`."#;
}

/// Rows a page shows for a rule another page defines: (page, rule, the row's text as the page has it).
pub const CROSS_REFS: &[(&str, Rule, &str)] = &[
    ("cars.md", Rule::Mrg3, r#"(from [layering.md](layering.md)) Every operation §4 refuses, with the field named."#),
    (
        "layering.md",
        Rule::Conv15,
        r#"(from [conventions.md](conventions.md) §11) A new name colliding by hash with another name of the same kind."#,
    ),
];
