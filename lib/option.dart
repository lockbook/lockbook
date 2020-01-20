abstract class Option<Value> {
  Option<New> map<New>(New Function(Value));

  Option<New> flatMap<New>(Option<New> Function(Value));

  Value getOrElse(Value value);
}

class Some<Value> extends Option<Value> {
  final Value _value;

  Some(this._value);

  @override
  Option<New> flatMap<New>(Option<New> Function(Value) next) => next(_value);

  @override
  Value getOrElse(Value value) => _value;

  @override
  Option<New> map<New>(New Function(Value) next) => Some(next(_value));
}

class None<Value> extends Option<Value> {
  None();

  @override
  Option<New> flatMap<New>(Option<New> Function(Value) Function) => None();

  @override
  Value getOrElse(Value value) => value;

  @override
  Option<New> map<New>(New Function(Value) Function) => None();
}
