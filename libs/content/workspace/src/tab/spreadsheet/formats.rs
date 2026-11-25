use std::io::Cursor;
use std::path::Path;

use ironcalc_base::types::Workbook;
use ironcalc_base::{Model, UserModel};
use lb_rs::model::file_metadata::DocumentHmac;

use super::{COLUMNS, MAX_EDIT_CELLS, ROWS, Spreadsheet};

// Version 1 uses the workbook encoding of ironcalc_base 0.9.1.
const HEADER: &[u8] = b"LBSHEET\0\x01";
pub(crate) const MAX_FILE_BYTES: usize = 100 * 1024 * 1024;
pub(crate) const NATIVE_EXTENSION: &str = "lbsheet";

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum FileFormat {
    Lbsheet,
    Csv,
    Xlsx,
}

impl FileFormat {
    pub(super) fn from_name(name: &str) -> Option<Self> {
        let ext = name.rsplit_once('.')?.1;
        if ext.eq_ignore_ascii_case(NATIVE_EXTENSION) {
            Some(Self::Lbsheet)
        } else if ext.eq_ignore_ascii_case("csv") {
            Some(Self::Csv)
        } else if ext.eq_ignore_ascii_case("xlsx") {
            Some(Self::Xlsx)
        } else {
            None
        }
    }

    pub(super) fn read(
        self, hmac: Option<DocumentHmac>, name: &str, bytes: &[u8],
    ) -> Result<UserModel<'static>, String> {
        if bytes.len() > MAX_FILE_BYTES {
            return Err("Spreadsheet files are limited to 100 MB".into());
        }
        match self {
            Self::Lbsheet => read_workbook(hmac, bytes),
            Self::Csv => import_csv(bytes).map(UserModel::from_model),
            Self::Xlsx => {
                let workbook = ironcalc::import::load_from_xlsx_bytes(bytes, name, "en", "UTC")
                    .map_err(|error| error.user_message())?;
                validate_workbook(&workbook)?;
                Model::from_workbook(workbook, "en").map(UserModel::from_model)
            }
        }
    }
}

pub(crate) fn is_import_name(name: &str) -> bool {
    matches!(FileFormat::from_name(name), Some(FileFormat::Csv | FileFormat::Xlsx))
}

pub(crate) fn encode_workbook(workbook: &Workbook) -> Vec<u8> {
    let mut bytes = HEADER.to_vec();
    bytes.extend(bitcode::encode(workbook));
    bytes
}

pub(super) fn read_workbook(
    hmac: Option<DocumentHmac>, bytes: &[u8],
) -> Result<UserModel<'static>, String> {
    if bytes.is_empty() && hmac.is_none() {
        return UserModel::new_empty("Spreadsheet", "en", "UTC", "en");
    }
    let payload = bytes
        .strip_prefix(HEADER)
        .ok_or("Unsupported spreadsheet format")?;
    let workbook =
        bitcode::decode(payload).map_err(|error| format!("Could not open spreadsheet: {error}"))?;
    validate_workbook(&workbook)?;
    Model::from_workbook(workbook, "en").map(UserModel::from_model)
}

fn validate_workbook(workbook: &Workbook) -> Result<(), String> {
    if workbook.worksheets.is_empty() {
        return Err("Spreadsheet has no worksheets".into());
    }
    Ok(())
}

fn import_csv(bytes: &[u8]) -> Result<Model<'static>, String> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(bytes);
    let mut model = Model::new_empty("Imported CSV", "en", "UTC", "en")?;
    let mut cell_count = 0;
    for (row, record) in reader.records().enumerate() {
        let record = record.map_err(|error| error.to_string())?;
        cell_count += record.len();
        if row >= ROWS as usize || record.len() > COLUMNS as usize {
            return Err("The CSV data does not fit in a worksheet".into());
        }
        if cell_count > MAX_EDIT_CELLS as usize {
            return Err("Too many cells to import at once".into());
        }
        for (column, value) in record
            .iter()
            .enumerate()
            .filter(|(_, value)| !value.is_empty())
        {
            model.set_user_input(0, row as i32 + 1, column as i32 + 1, value.to_owned())?;
        }
    }
    model.evaluate();
    Ok(model)
}

impl Spreadsheet {
    pub(super) fn export_csv(&self) -> Result<(String, Vec<u8>), String> {
        let sheet = self.worksheet();
        let dimension = sheet.dimension();
        let rows = dimension.max_row.max(1);
        let columns = dimension.max_column.max(1);
        if i64::from(rows) * i64::from(columns) > MAX_EDIT_CELLS {
            return Err("This worksheet is too large to export as CSV".into());
        }
        let mut writer = csv::Writer::from_writer(Vec::new());
        for row in 1..=rows {
            let fields = (1..=columns)
                .map(|column| self.model.get_formatted_cell_value(self.sheet, row, column))
                .collect::<Result<Vec<_>, _>>()?;
            writer
                .write_record(fields)
                .map_err(|error| error.to_string())?;
        }
        let bytes = writer.into_inner().map_err(|error| error.to_string())?;
        // Imported sheet names can contain characters rejected by file pickers or C strings.
        let title = sheet
            .name
            .replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|', '\0'], "_");
        let name = Path::new(&title)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("Spreadsheet");
        Ok((format!("{name}.csv"), bytes))
    }

    pub(super) fn export_xlsx(&self) -> Result<(String, Vec<u8>), String> {
        let writer =
            ironcalc::export::save_xlsx_to_writer(self.model.get_model(), Cursor::new(Vec::new()))
                .map_err(|error| error.user_message())?;
        Ok(("Spreadsheet.xlsx".into(), writer.into_inner()))
    }
}
