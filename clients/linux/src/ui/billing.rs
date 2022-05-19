use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;

use crate::ui;

#[derive(Clone)]
pub struct UpgradePaymentFlow {
    header: UpgradeHeader,
    payment_method: SelectPayMethod,
    confirm_details: ConfirmDetails,
    pages: gtk::Stack,
    pub cntr: gtk::Box,
}

impl UpgradePaymentFlow {
    pub fn new(maybe_card: Option<lb::CreditCardLast4Digits>) -> Self {
        let header = UpgradeHeader::new();
        header.payment_method.mark_active();

        let payment_method = SelectPayMethod::new(maybe_card);
        let confirm_details = ConfirmDetails::new();

        let pages = gtk::Stack::new();
        pages.set_margin_start(12);
        pages.set_margin_end(12);
        pages.set_margin_bottom(12);
        pages.add_named(&payment_method.cntr, Some("payment_method"));
        pages.add_named(&confirm_details.cntr, Some("confirm_details"));

        payment_method.connect_method_selected({
            let header = header.clone();
            let pages = pages.clone();
            let confirm_details = confirm_details.clone();

            move |method| {
                confirm_details.set_for_payment_method(method);
                header.payment_method.mark_complete();
                header.confirm_details.mark_active();
                pages.set_visible_child_name("confirm_details");
            }
        });

        confirm_details.btn_go_back.connect_clicked({
            let header = header.clone();
            let pages = pages.clone();

            move |_| {
                header.payment_method.mark_active();
                header.confirm_details.mark_incomplete();
                pages.set_visible_child_name("payment_method");
            }
        });

        let cntr = gtk::Box::new(gtk::Orientation::Vertical, 0);
        cntr.append(&header.cntr);
        cntr.append(&pages);

        Self { header, payment_method, confirm_details, pages, cntr }
    }

    pub fn connect_cancelled<F: Fn(&Self) + 'static>(&self, f: F) {
        let this = self.clone();
        self.payment_method
            .btn_cancel
            .connect_clicked(move |_| f(&this));
    }

    pub fn connect_confirmed<F: Fn(&Self, lb::PaymentMethod) + 'static>(&self, f: F) {
        let this = self.clone();
        self.confirm_details.btn_confirm.connect_clicked(move |_| {
            let method = this.confirm_details.method.borrow_mut().take().unwrap();
            f(&this, method);
        });
    }

    pub fn show_pay_screen<W: IsA<gtk::Widget>>(&self, payment_ui: &W) {
        self.header.confirm_details.mark_complete();
        self.header.pay_and_upgrade.mark_active();
        self.pages.add_named(payment_ui, Some("pay_and_upgrade"));
        self.pages.set_visible_child_name("pay_and_upgrade");
    }

    pub fn mark_final_header_section_complete(&self) {
        self.header.pay_and_upgrade.mark_complete();
    }

    pub fn set_final_header_icon(&self, name: &str) {
        self.header.pay_and_upgrade.icon.set_icon_name(Some(name));
    }
}

#[derive(Clone)]
struct UpgradeHeader {
    payment_method: HeaderSection,
    confirm_details: HeaderSection,
    pay_and_upgrade: HeaderSection,
    cntr: gtk::Box,
}

impl UpgradeHeader {
    fn new() -> Self {
        let payment_method = HeaderSection::new("Payment Method");
        let confirm_details = HeaderSection::new("Confirm Details");
        let pay_and_upgrade = HeaderSection::new("Pay and Upgrade");

        let steps = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        steps.set_hexpand(true);
        steps.append(&payment_method.cntr);
        steps.append(&gtk::Separator::new(gtk::Orientation::Vertical));
        steps.append(&gtk::Separator::new(gtk::Orientation::Vertical));
        steps.append(&confirm_details.cntr);
        steps.append(&gtk::Separator::new(gtk::Orientation::Vertical));
        steps.append(&gtk::Separator::new(gtk::Orientation::Vertical));
        steps.append(&pay_and_upgrade.cntr);

        let cntr = gtk::Box::new(gtk::Orientation::Vertical, 0);
        cntr.add_css_class("upgrade-header");
        cntr.set_margin_bottom(24);
        cntr.append(&steps);
        cntr.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        cntr.append(&gtk::Separator::new(gtk::Orientation::Horizontal));

        Self { payment_method, confirm_details, pay_and_upgrade, cntr }
    }
}

#[derive(Clone)]
struct HeaderSection {
    icon: gtk::Image,
    title: gtk::Label,
    cntr: gtk::Box,
}

impl HeaderSection {
    fn new(text: &str) -> Self {
        let icon = gtk::Image::from_icon_name("action-unavailable-symbolic");
        icon.add_css_class("grayed-out");
        icon.set_margin_top(12);
        icon.set_pixel_size(20);

        let title = gtk::Label::new(Some(text));
        title.set_margin_bottom(16);
        title.set_sensitive(false);

        let cntr = gtk::Box::new(gtk::Orientation::Vertical, 12);
        cntr.set_hexpand(true);
        cntr.append(&icon);
        cntr.append(&title);

        Self { icon, title, cntr }
    }

    fn mark_active(&self) {
        self.icon.set_icon_name(Some("dialog-information-symbolic"));
        self.icon.remove_css_class("grayed-out");
        self.icon.remove_css_class("green");
        self.title.set_sensitive(true);
        self.cntr.add_css_class("upgrade-header-active-section");
    }

    fn mark_complete(&self) {
        self.icon.set_icon_name(Some("emblem-ok-symbolic"));
        self.icon.add_css_class("green");
        self.title.set_sensitive(false);
    }

    fn mark_incomplete(&self) {
        self.icon.set_icon_name(Some("action-unavailable-symbolic"));
        self.icon.remove_css_class("green");
        self.icon.add_css_class("grayed-out");
        self.title.set_sensitive(false);
        self.cntr.remove_css_class("upgrade-header-active-section");
    }
}

#[derive(Clone)]
pub struct SelectPayMethod {
    old_card: gtk::CheckButton,
    new_card: gtk::CheckButton,
    new_card_input: CreditCardInput,
    btn_cancel: gtk::Button,
    btn_continue: gtk::Button,
    pub cntr: gtk::Box,
}

impl SelectPayMethod {
    pub fn new(maybe_existing_card: Option<lb::CreditCardLast4Digits>) -> Self {
        let group = gtk::CheckButton::new();

        let old_card = gtk::CheckButton::new();

        let new_card_input = CreditCardInput::new();
        let new_card = gtk::CheckButton::with_label("New Card");

        let methods = gtk::Box::new(gtk::Orientation::Vertical, 8);
        methods.set_vexpand(true);

        if let Some(card_last4) = maybe_existing_card {
            old_card.set_label(Some(&format!("Current Card ({})", card_last4)));
            old_card.set_group(Some(&group));
            old_card.set_active(true);

            new_card.set_group(Some(&group));
            new_card.connect_toggled({
                let new_card_input = new_card_input.clone();
                move |btn| {
                    new_card_input.revealer.set_reveal_child(btn.is_active());
                    if btn.is_active() {
                        new_card_input.number.grab_focus();
                    }
                }
            });

            methods.append(&old_card);
            methods.append(&new_card);
            methods.append(&new_card_input.revealer);
        } else {
            new_card.set_active(true);
            new_card_input.revealer.set_reveal_child(true);
            new_card_input.number.grab_focus();

            methods.append(&gtk::Label::new(Some("Enter your credit card info:")));
            methods.append(&new_card_input.revealer);
        }

        let btn_cancel = prev_button("Cancel");
        let btn_continue = next_button("Continue");

        let cntr = gtk::Box::new(gtk::Orientation::Vertical, 8);
        cntr.append(&methods);
        cntr.append(&btn_grid(&[&btn_cancel, &btn_continue]));

        Self { old_card, new_card, new_card_input, btn_cancel, btn_continue, cntr }
    }

    fn payment_method(&self) -> Option<lb::PaymentMethod> {
        if self.new_card.is_active() {
            return match self.new_card_input.info() {
                Ok(info) => Some(lb::PaymentMethod::NewCard {
                    number: info.number,
                    cvc: info.cvc,
                    exp_month: info.exp_month,
                    exp_year: info.exp_year,
                }),
                Err(err) => {
                    self.new_card_input.handle_err(err);
                    None
                }
            };
        }
        if self.old_card.is_active() {
            return Some(lb::PaymentMethod::OldCard);
        }
        None
    }

    fn connect_method_selected<F: Fn(lb::PaymentMethod) + 'static>(&self, f: F) {
        let this = self.clone();
        self.btn_continue.connect_clicked(move |_| {
            if let Some(method) = this.payment_method() {
                f(method);
            }
        });
    }
}

#[derive(Clone)]
struct ConfirmDetails {
    method: Rc<RefCell<Option<lb::PaymentMethod>>>,
    content: gtk::Box,
    btn_go_back: gtk::Button,
    btn_confirm: gtk::Button,
    cntr: gtk::Box,
}

impl ConfirmDetails {
    fn new() -> Self {
        let method = Rc::new(RefCell::new(None));

        let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
        content.set_vexpand(true);

        let btn_go_back = prev_button("Go Back");
        let btn_confirm = next_button("Confirm");

        let cntr = gtk::Box::new(gtk::Orientation::Vertical, 0);
        cntr.append(&content);
        cntr.append(&btn_grid(&[&btn_go_back, &btn_confirm]));

        Self { method, content, btn_go_back, btn_confirm, cntr }
    }

    fn set_for_payment_method(&self, method: lb::PaymentMethod) {
        *self.method.borrow_mut() = Some(method);
        ui::clear(&self.content);

        let prompt = gtk::Label::builder()
            .label(
                "You are about to pay\n$2.50 per month via credit card\nfor 50 gigabytes of space.",
            )
            .justify(gtk::Justification::Center)
            .margin_bottom(24)
            .build();
        self.content.append(&prompt);
    }
}

struct CardInfo {
    number: String,
    cvc: String,
    exp_month: i32,
    exp_year: i32,
}

enum CardError {
    Number,
    Cvc,
    ExpMonth,
    ExpYear,
}

#[derive(Clone)]
struct CreditCardInput {
    number: gtk::Entry,
    exp_month: gtk::Entry,
    exp_year: gtk::Entry,
    cvc: gtk::Entry,
    error: gtk::Label,
    revealer: gtk::Revealer,
}

impl CreditCardInput {
    fn new() -> Self {
        let error = gtk::Label::new(None);
        error.add_css_class("err");
        error.hide();

        let card_input_entry = {
            let error = error.clone();

            move |placeholder: &str| {
                let entry = gtk::Entry::new();
                entry.set_placeholder_text(Some(placeholder));
                entry.connect_changed({
                    let error = error.clone();

                    move |entry| {
                        entry.remove_css_class("err-input");
                        error.hide();
                    }
                });
                entry
            }
        };

        let number = card_input_entry("Card Number");
        let exp_month = card_input_entry("MM");
        let exp_year = card_input_entry("YY");
        let cvc = card_input_entry("CVC");

        number.set_width_request(260);

        let inputs = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        inputs.append(&number);
        inputs.append(&exp_month);
        inputs.append(&exp_year);
        inputs.append(&cvc);

        let cntr = gtk::Box::new(gtk::Orientation::Vertical, 8);
        cntr.append(&inputs);
        cntr.append(&error);

        let revealer = gtk::Revealer::builder().child(&cntr).build();

        Self { number, cvc, exp_month, exp_year, error, revealer }
    }

    fn info(&self) -> Result<CardInfo, CardError> {
        let number = self.number.text().to_string();
        if number.len() < 14 || number.len() > 16 {
            return Err(CardError::Number);
        }

        let cvc = self.cvc.text().to_string();
        if cvc.len() < 3 {
            return Err(CardError::Cvc);
        }

        let exp_month: i32 = self
            .exp_month
            .text()
            .to_string()
            .parse()
            .map_err(|_| CardError::ExpMonth)?;
        if !(1..=12).contains(&exp_month) {
            return Err(CardError::ExpMonth);
        }

        let exp_year: i32 = self
            .exp_year
            .text()
            .to_string()
            .parse()
            .map_err(|_| CardError::ExpYear)?;

        Ok(CardInfo { number, cvc, exp_month, exp_year })
    }

    fn handle_err(&self, err: CardError) {
        let (entry, msg) = match err {
            CardError::Number => (&self.number, "Please enter a valid card number."),
            CardError::Cvc => (&self.cvc, "Please enter a valid CVC."),
            CardError::ExpMonth => (&self.exp_month, "Please enter a valid expiry month."),
            CardError::ExpYear => (&self.exp_year, "Please enter a valid expiry year."),
        };
        entry.add_css_class("err-input");
        self.error.set_text(msg);
        self.error.show();
    }
}

fn btn_grid(btns: &[&gtk::Button]) -> gtk::Grid {
    let bg = gtk::Grid::builder()
        .column_spacing(4)
        .column_homogeneous(true)
        .build();
    for (i, btn) in btns.iter().enumerate() {
        bg.attach(*btn, i as i32, 0, 1, 1);
    }
    bg
}

fn prev_button(label: &str) -> gtk::Button {
    let left_arrow = gtk::Image::from_icon_name("go-previous-symbolic");
    let text = gtk::Label::new(Some(label));

    let content = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    content.set_halign(gtk::Align::Center);
    content.append(&left_arrow);
    content.append(&text);

    gtk::Button::builder().child(&content).build()
}

fn next_button(label: &str) -> gtk::Button {
    let right_arrow = gtk::Image::from_icon_name("go-next-symbolic");
    let text = gtk::Label::new(Some(label));

    let content = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    content.set_halign(gtk::Align::Center);
    content.append(&text);
    content.append(&right_arrow);

    gtk::Button::builder().child(&content).build()
}
