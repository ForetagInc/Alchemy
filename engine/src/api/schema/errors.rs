use juniper::{FieldError, IntoFieldError, ScalarValue, Value};

pub struct NotFoundError {
	model: String,
}

impl NotFoundError {
	pub fn new(model: String) -> Self {
		Self { model }
	}
}

impl<S: ScalarValue> IntoFieldError<S> for NotFoundError {
	fn into_field_error(self) -> FieldError<S> {
		FieldError::new(format!("{} not found", self.model), Value::Null)
	}
}

pub struct DatabaseError {
	message: String,
}

impl DatabaseError {
	pub fn new(message: String) -> Self {
		Self { message }
	}
}

impl<S: ScalarValue> IntoFieldError<S> for DatabaseError {
	fn into_field_error(self) -> FieldError<S> {
		FieldError::new(self.message, Value::Null)
	}
}
