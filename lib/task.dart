import 'package:client/errors.dart';

enum Empty {
  Nothing,
}

const Done = Empty.Nothing;

abstract class Either<Error extends UIError, Value> {
  Either<Error, New> flatMap<New>(Either<Error, New> next(Value input));

  Either<Error, New> map<New>(New conversion(Value input));

  Either<Error, Value> ifSuccessDo(sideEffect(Value input));

  Either<Error, Value> ifFailedDo(sideEffect(Error input));

  Value getValueUnsafely();

  bool isSuccessful();

  Future<Either<Error, New>> flatMapFut<New>(
      Future<Either<Error, New>> next(Value input));
}

class Success<Error extends UIError, Value> extends Either<Error, Value> {
  final Value _value;

  Success(this._value);

  @override
  Either<Error, New> flatMap<New>(Either<Error, New> Function(Value input) next) =>
      next(_value);

  @override
  Future<Either<Error, New>> flatMapFut<New>(
          Future<Either<Error, New>> Function(Value input) next) =>
      next(_value).catchError((error) => Fail(unhandledError(error)));

  @override
  Either<Error, New> map<New>(New Function(Value) conversion) =>
      Success(conversion(_value));

  @override
  Either<Error, Value> ifFailedDo(Function(Error input) sideEffect) =>
      Success(_value);

  @override
  Either<Error, Value> ifSuccessDo(Function(Value input) sideEffect) {
    sideEffect(_value);
    return Success(_value);
  }

  @override
  Value getValueUnsafely() => _value;

  @override
  bool isSuccessful() => true;
}

class Fail<Error extends UIError, Value> extends Either<Error, Value> {
  final Error _error;

  Fail(this._error);

  @override
  Either<Error, New> flatMap<New>(Either<Error, New> Function(Value input) next) =>
      Fail(_error);

  @override
  Future<Either<Error, New>> flatMapFut<New>(
          Future<Either<Error, New>> Function(Value input) next) =>
      Future.value(Fail(_error));

  @override
  Either<Error, New> map<New>(New Function(Value) conversion) => Fail(_error);

  @override
  Either<Error, Value> ifFailedDo(Function(Error input) sideEffect) {
    sideEffect(_error);
    return Fail(_error);
  }

  @override
  Either<Error, Value> ifSuccessDo(Function(Value input) sideEffect) =>
      Fail(_error);

  @override
  Value getValueUnsafely() {
    throw NullThrownError();
  }

  @override
  bool isSuccessful() => false;
}
