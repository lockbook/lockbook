import 'package:client/db_provider.dart';
import 'package:client/errors.dart';
import 'package:sqflite/sqflite.dart';

import 'either.dart';

class LastUpdateRepository {
  final DBProvider _dbProvider;

  const LastUpdateRepository(this._dbProvider);

  Future<Either<UIError, BigInt>> getLastUpdated() async {
    final connected = await _dbProvider.connectToDB();

    final stringTimestamp = await connected.flatMapFut(_getLastUpdatedQuery);

    final parsed = stringTimestamp.map(BigInt.parse);

    return parsed;
  }

  Future<Either<UIError, String>> _getLastUpdatedQuery(Database db) async {
    final list = await db.rawQuery('select * from LastUpdated');

    if (list.length == 1) {
      final row = list[0];
      return Success(row['timestamp'] as String);
    } else {
      return Success('0');
    }
  }
}
