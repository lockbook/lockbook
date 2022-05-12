use gtk::prelude::*;

#[derive(Clone)]
pub struct UpgradePanel {
    new_card: gtk::CheckButton,
    btc: gtk::CheckButton,
    xmr: gtk::CheckButton,
    pub cc_input: CreditCardInput,
    pub btn_continue: gtk::Button,
    pub steps: gtk::Stack,
}

impl UpgradePanel {
    pub fn new(maybe_card: Option<lb::CreditCardLast4Digits>) -> Self {
        let heading = gtk::Label::builder()
            .label("Please select a payment method:")
            .css_classes(vec!["settings-heading".to_string()])
            .halign(gtk::Align::Start)
            .margin_bottom(8)
            .build();

        let methods = gtk::Box::new(gtk::Orientation::Vertical, 8);
        methods.set_vexpand(true);
        methods.append(&heading);

        let group = gtk::CheckButton::new();

        let cc_input = CreditCardInput::new();

        let new_card = gtk::CheckButton::with_label("New Card");
        new_card.set_group(Some(&group));
        new_card.connect_toggled({
            let cc_input = cc_input.clone();
            move |btn| {
                cc_input.revealer.set_reveal_child(btn.is_active());
                if btn.is_active() {
                    cc_input.number.grab_focus();
                }
            }
        });

        let new_card_cntr = gtk::Box::new(gtk::Orientation::Vertical, 0);
        new_card_cntr.append(&new_card);
        new_card_cntr.append(&cc_input.revealer);

        if let Some(current_card_last4) = maybe_card {
            let current_card =
                gtk::CheckButton::with_label(&format!("Currrent Card ({})", current_card_last4));
            current_card.set_active(true);
            methods.append(&current_card);
        } else {
            //new_card.emit_activate();
        }

        let xmr = gtk::CheckButton::with_label("Monero");
        xmr.set_group(Some(&group));

        let btc = gtk::CheckButton::with_label("Bitcoin");
        btc.set_group(Some(&group));

        let btn_cancel = gtk::Button::with_label("Cancel");
        let btn_continue = gtk::Button::with_label("Continue");

        let buttons = gtk::Grid::builder()
            .column_spacing(4)
            .column_homogeneous(true)
            .margin_bottom(12)
            .build();
        buttons.attach(&btn_cancel, 0, 0, 1, 1);
        buttons.attach(&btn_continue, 1, 0, 1, 1);

        methods.append(&new_card_cntr);
        methods.append(&xmr);
        methods.append(&btc);

        let cntr = gtk::Box::new(gtk::Orientation::Vertical, 12);
        cntr.append(&methods);
        cntr.append(&buttons);

        let steps = gtk::Stack::new();
        steps.add_named(&cntr, Some("methods"));

        Self { new_card, btc, xmr, cc_input, btn_continue, steps }
    }

    pub fn input_to_payment_method(&self) -> Option<lb::PaymentMethod> {
        if self.new_card.is_active() {
            match self.cc_input.info() {
                Ok(info) => {
                    return Some(lb::PaymentMethod::NewCard {
                        number: info.number,
                        cvc: info.cvc,
                        exp_month: info.exp_month,
                        exp_year: info.exp_year,
                    })
                }
                Err(err) => self.cc_input.handle_err(err),
            }
        }
        if self.btc.is_active() {
            eprintln!("todo");
        }
        if self.xmr.is_active() {
            eprintln!("todo");
        }
        None
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
pub struct CreditCardInput {
    number: gtk::Entry,
    cvc: gtk::Entry,
    exp_month: gtk::Entry,
    exp_year: gtk::Entry,
    error: gtk::Label,
    revealer: gtk::Revealer,
}

impl CreditCardInput {
    fn new() -> Self {
        let number = gtk::Entry::new();
        number.set_placeholder_text(Some("Card Number"));
        number.set_hexpand(true);
        number.set_width_request(260);

        let cvc = gtk::Entry::new();
        cvc.set_placeholder_text(Some("CVC"));

        let exp_month = gtk::Entry::new();
        exp_month.set_placeholder_text(Some("MM"));

        let exp_year = gtk::Entry::new();
        exp_year.set_placeholder_text(Some("YY"));

        let cvc_and_expiry = gtk::Grid::builder()
            .column_spacing(4)
            .column_homogeneous(true)
            .build();
        cvc_and_expiry.attach(&cvc, 0, 0, 1, 1);
        cvc_and_expiry.attach(&exp_month, 1, 0, 1, 1);
        cvc_and_expiry.attach(&exp_year, 2, 0, 1, 1);

        let inputs = gtk::Grid::builder()
            .column_spacing(4)
            .margin_top(8)
            .margin_start(12)
            .margin_end(12)
            .build();
        inputs.attach(&number, 0, 0, 1, 1);
        inputs.attach(&cvc_and_expiry, 1, 0, 1, 1);

        let error = gtk::Label::new(None);
        error.add_css_class("err");
        error.hide();

        let cntr = gtk::Box::new(gtk::Orientation::Vertical, 8);
        cntr.append(&inputs);
        cntr.append(&error);

        let revealer = gtk::Revealer::builder().child(&cntr).build();

        Self {
            number,
            cvc,
            exp_month,
            exp_year,
            error,
            revealer,
        }
    }

    fn info(&self) -> Result<CardInfo, CardError> {
        let number = self.number.text().to_string();
        if number.len() < 14 || number.len() > 16 {
            return Err(CardError::Number);
        }

        let cvc = self.cvc.text().to_string();
        if cvc.len() < 16 {
            return Err(CardError::Cvc);
        }

        let exp_month: i32 = self
            .exp_month
            .text()
            .to_string()
            .parse()
            .map_err(|_| CardError::ExpMonth)?;

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
