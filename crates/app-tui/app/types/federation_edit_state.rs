use sotf_audio_player::federation_config::FederationSourceEntry;

#[derive(Debug, Clone)]
pub struct FederationEditState {
    pub source: FederationSourceEntry,
    /// Field index within the source-specific connection fields
    /// 0..N are connection fields, N is display_name, N+1 is priority, N+2 is enabled
    pub selected_field: usize,
    pub editing_value: bool,
    pub edit_buffer: String,
    pub is_new: bool,
}

impl FederationEditState {
    pub fn new(source: FederationSourceEntry, is_new: bool) -> Self {
        Self {
            source,
            selected_field: 0,
            editing_value: false,
            edit_buffer: String::new(),
            is_new,
        }
    }

    /// Total number of editable fields (connection fields + name + priority)
    pub fn field_count(&self) -> usize {
        self.source.connection.field_names().len() + 2
    }

    /// Get label for the field at the given index
    pub fn field_label(&self, index: usize) -> &str {
        let conn_fields = self.source.connection.field_names();
        if index < conn_fields.len() {
            conn_fields[index]
        } else if index == conn_fields.len() {
            "Display Name"
        } else {
            "Priority"
        }
    }

    /// Get value for the field at the given index
    pub fn field_value(&self, index: usize) -> String {
        let conn_fields = self.source.connection.field_names();
        if index < conn_fields.len() {
            self.source.connection.field_value(index)
        } else if index == conn_fields.len() {
            self.source.display_name.clone()
        } else {
            self.source.priority.to_string()
        }
    }

    /// Set value for the field at the given index
    pub fn set_field_value(&mut self, index: usize, value: &str) {
        let conn_field_count = self.source.connection.field_names().len();
        if index < conn_field_count {
            self.source.connection.set_field_value(index, value);
        } else if index == conn_field_count {
            self.source.display_name = value.to_string();
        } else if let Ok(p) = value.parse() {
            self.source.priority = p;
        }
    }
}
