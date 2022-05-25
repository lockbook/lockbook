use gtk::prelude::*;

pub struct CardInfo {
    pub number: String,
    pub cvc: String,
    pub exp_month: i32,
    pub exp_year: i32,
}

pub enum CardError {
    Number,
    Cvc,
    ExpMonth,
    ExpYear,
}

#[derive(Clone)]
pub struct CreditCardInput {
    pub number: gtk::Entry,
    exp_month: gtk::Entry,
    exp_year: gtk::Entry,
    cvc: gtk::Entry,
    error: gtk::Label,
    pub revealer: gtk::Revealer,
}

impl CreditCardInput {
    pub fn new() -> Self {
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

    pub fn info(&self) -> Result<CardInfo, CardError> {
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

    pub fn handle_err(&self, err: CardError) {
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
