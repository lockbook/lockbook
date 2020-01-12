import 'package:client/errors.dart';

// One day
// typedef AsyncTask = Future<Task>;

/// An Either-like abstraction to try to curtail:
/// https://journal.stuffwithstuff.com/2015/02/01/what-color-is-your-function/
abstract class Task<Error extends UIError, Value> {
  /// Traditionally a flatMap
  Task<Error, New> thenDo<New>(Task<Error, New> next(Value input));

  /// Traditionally a map
  Task<Error, New> convertValue<New>(New conversion(Value input));

  Task<Error, Value> ifSuccess(sideEffect(Value input));

  Task<Error, Value> ifFailure(sideEffect(Error input));

  /// Given you subscribe to the ideas of Monadic error handling:
  /// This function is the reason this file exists if you were
  /// to use something like Dartz' Either you'd have situations where you'll
  /// try to chain several Future<Either> calls you can await the
  /// first one, but then you can't await inside the subsequent flatmap
  ///
  /// Maybe you'll try to do Future.toStream chains you'll rapidly run into
  /// A Future<Either<E, Future<Eiither>>> type situation
  ///
  /// Maybe you'll try something like Dartz's Task, and you'll have
  /// Task<Either> and will need some adapters between flatmaps, but you'll
  /// need to adapt each either for every flatmap. This can be cumbersome and
  /// you'll lose some typesafety when you use Task.fail. Which was the initial
  /// motivation for using an Either!
  ///
  /// This guy will automatically adapt Failed futures to a predefined ErrorType
  /// in our type hierarchy, and our "adapter" becomes `await`.
  /// This is not perfect, because it makes ordering the chain your responsibility.
  /// But it seems to be the local optimum for me.
  Future<Task<Error, New>> thenDoFuture<New>(
      Future<Task<Error, New>> next(Value input));
}

class Success<Error extends UIError, Value> extends Task<Error, Value> {
  final Value _value;

  Success(this._value);

  @override
  Task<Error, New> thenDo<New>(Task<Error, New> Function(Value input) next) =>
      next(_value);

  @override
  Future<Task<Error, New>> thenDoFuture<New>(
          Future<Task<Error, New>> Function(Value input) next) =>
      next(_value).catchError((error) => Fail(unhandledError(error)));

  @override
  Task<Error, New> convertValue<New>(New Function(Value) conversion) =>
      Success(conversion(_value));

  @override
  Task<Error, Value> ifFailure(Function(Error input) sideEffect) =>
      Success(_value);

  @override
  Task<Error, Value> ifSuccess(Function(Value input) sideEffect) {
    sideEffect(_value);
    return Success(_value);
  }
}

class Fail<Error extends UIError, Value> extends Task<Error, Value> {
  final Error _error;

  Fail(this._error);

  @override
  Task<Error, New> thenDo<New>(Task<Error, New> Function(Value input) next) =>
      Fail(_error);

  @override
  Future<Task<Error, New>> thenDoFuture<New>(
          Future<Task<Error, New>> Function(Value input) next) =>
      Future.value(Fail(_error));

  @override
  Task<Error, New> convertValue<New>(New Function(Value) conversion) =>
      Fail(_error);

  @override
  Task<Error, Value> ifFailure(Function(Error input) sideEffect) {
    sideEffect(_error);
    return Fail(_error);
  }

  @override
  Task<Error, Value> ifSuccess(Function(Value input) sideEffect) =>
      Fail(_error);
}
