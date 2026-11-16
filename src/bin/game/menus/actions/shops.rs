use crate::*;

impl Game {
    /// `act`'s customize / shop group (as `act_main_options`).
    pub(super) fn act_shops(&mut self, action: Action) -> Result<Option<Mode>> {
        Ok(Some(match action {
            // Customize (0x4AED30): game mode 0x20, UI_QRCarSelect.
            Action::Customize => {
                self.race_flow.game_mode = 0x20;
                self.customize_select()
            }
            // The categories (0x520D70's options) open the shops on the
            // car.
            Action::CustomizeCategory(c) => match c {
                0 => {
                    self.shop.garage_parts &= self.save_flow.customize.is_none();
                    self.new_parts_browser(0)
                }
                1 => self.icon_menu(MenuKind::PerfCategory),
                2 => self.icon_menu(MenuKind::Graphics),
                _ => self.specialties_browser(0, None),
            },
            // Change Parts in the garage: the body categories over the
            // stock and owned parts.
            // UI_ChooseCustomizeCategory in customize mode 1 (0x83898C;
            // Wine, the finished-career save): "Parts Storage" (0xE1497A54 on
            // 0x42ADB44C, 0x556747) over Change Body Parts, Change
            // Performance Parts and Change Interiors / Car Specialties
            // (0x520D70 leaves out Paint / Vinyls and the dyno in mode 1).
            Action::ChangeParts => {
                self.shop.garage_parts = true;
                self.icon_menu(MenuKind::CustomizeCategory)
            }
            Action::RimBrand(i) => self.rims_browser(i, None, None),
            Action::Paint => self.paint_shop(0, None, None),
            Action::Vinyls => self.vinyl_layer_menu(0),
            Action::Decals => {
                self.shop.decal_node = 0;
                self.decal_shop(0, 0, None)
            }
            // 0x562BA0: with a setup, "Would you like to purchase this
            // graphics setup for your car?" (career 0xA0C16CAB, else
            // 0x07F24787); without one, "Please make a selection to
            // install before checking out!" (0x51E3F647).
            Action::PurchaseSetup => {
                if self.shop.setup.is_empty() {
                    let text = self.label(0x51E3_F647, "Please make a selection to install before checking out!");
                    self.dialog(&text, self.graphics_menu(3))
                } else {
                    let (yes, no) = self.yes_no();
                    let question = if self.profile.is_some() { 0xA0C1_6CAB } else { 0x07F2_4787 };
                    let (a, b) = (self.graphics_menu(3), self.graphics_menu(3));
                    self.setup_dialog(
                        question,
                        vec![(yes, Some(SetupStep::Purchase { leave: false }), a), (no, None, b)],
                    )
                }
            }
            // A layer (0x563110): with a vinyl on it (or a unique one on
            // the car or in the setup) Change / Remove / Cancel ("Would you
            // like to change or remove this vinyl?" 0x06D7902C; "Unique
            // vinyls require all four vinyl layers…" 0x80C151D5); else the
            // vinyl browser.
            Action::VinylLayer(layer) => {
                self.shop.vinyl_layer = layer;
                let unique = self.fitted_unique() || self.setup_unique();
                if unique || self.layer_has_vinyl(layer) {
                    let question = if unique { 0x80C1_51D5 } else { 0x06D7_902C };
                    let at = (layer - 0x40) as usize;
                    let buttons = vec![
                        (
                            self.label(0x548B_4385, "Change"),
                            Some(SetupStep::ChangeVinyl(layer)),
                            self.vinyl_layer_menu(at),
                        ),
                        (
                            self.label(0x7758_F62D, "Remove"),
                            Some(SetupStep::RemoveVinyl(layer)),
                            self.vinyl_layer_menu(at),
                        ),
                        (self.label(0x5413_89E5, "Cancel"), None, self.vinyl_layer_menu(at)),
                    ];
                    self.setup_dialog(question, buttons)
                } else {
                    self.vinyl_browser(layer, None, None)
                }
            }
            Action::PerfCategory(c) => {
                // 0x53B780 → 0x525050(c): table 0x5250E8 by the category.
                if let Some(&kind) = [0x10, 0x0C, 9, 7, 0x0D, 0x0A, 3, 6, 0x0B].get(c as usize) {
                    self.option_line(kind);
                }
                self.shop.perf_category = c;
                self.shop.perf_parts = Some(self.perf_parts_list(0));
                // On Choose Individual Parts, its first option: 0x556E70
                // pushes UI_BuyPerformanceParts and sets the strip's entry 1
                // (vt+0x20(1), 1-based as UI_ChooseRimBrand's 0x546530) in
                // the shops as in the storage (Wine, the finished-career save in
                // IND_PERF_SHOP_1: every category opened on it).
                self.perf_package_menu(0)
            }
            Action::PerfPackage(level) => self.perf_package_accept(level),
            Action::PerfStock => self.perf_stock_accept(),
            // Accept reaches UI_BuyPerformanceParts' row instead (menu_key).
            Action::PerfParts => return Ok(None),
            _ => unreachable!("act_shops: {action:?}"),
        }))
    }
}
