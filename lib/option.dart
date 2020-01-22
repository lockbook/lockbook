import 'either.dart';
import 'errors.dart';

abstract class Option<Value> {
  Option<New> map<New>(New Function(Value old));

  Option<New> flatMap<New>(Option<New> Function(Value old));

  Value getOrElse(Value value);

  Either<Error, Value> toEither<Error extends UIError>(Error callIfNone());
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

  @override
  Either<New, Value> toEither<New extends UIError>(New callIfNone()) => Success(_value);
}

class None<Value> extends Option<Value> {
  None();

  @override
  Option<New> flatMap<New>(Option<New> Function(Value) Function) => None();

  @override
  Value getOrElse(Value value) => value;

  @override
  Option<New> map<New>(New Function(Value) Function) => None();

  @override
  Either<New, Value> toEither<New extends UIError>(New callIfNone()) => Fail(callIfNone());
}
