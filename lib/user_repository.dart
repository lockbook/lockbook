import 'package:client/either.dart';
import 'package:client/user_info.dart';
import 'package:sqflite/sqlite_api.dart';

import 'db_provider.dart';
import 'errors.dart';

class UserRepository {
  final DBProvider _dbProvider;

  const UserRepository(this._dbProvider);

  // Perhaps an example where I'd like to programmatically differentiate between
  // "I can't access files" and "You don't have a username yet" -- TODO Should be an either of option
  Future<Either<UIError, UserInfo>> getUserInfo() async {
    return (await _dbProvider.connectToDB()).flatMapFut(_getUserInfo);
  }

  Future<Either<UIError, UserInfo>> _getUserInfo(Database db) async {
    final results = (await db.rawQuery('select * from UserInfo'));

    if (results.length == 1) {
      return UserInfo.fromMap(results[0]);
    } else {
      return Fail(noUserError());
    }
  }

  Future<Either<UIError, Empty>> saveUserInfo(UserInfo userInfo) async {
    return (await _dbProvider.connectToDB())
        .flatMapFut((db) => _saveUserInfo(db, userInfo));
  }

  Future<Either<UIError, Empty>> _saveUserInfo(
      Database db, UserInfo userInfo) async {
    int insert = await db.rawInsert('''REPLACE INTO 
        UserInfo(id, username, modulus, public_exponent, private_exponent, p, q) 
        VALUES(1, 
        "${userInfo.username.toString()}", 
        "${userInfo.modulus.toString()}", 
        "${userInfo.publicExponent.toString()}", 
        "${userInfo.privateExponent.toString()}", 
        "${userInfo.p.toString()}", 
        "${userInfo.q.toString()}")
        ''');

    print("here");

    if (insert == 1) {
      return Success(Done);
    } else {
      return Fail(noUserError());
    }
  }
}
