import 'dart:io';

import 'package:client/errors.dart';
import 'package:client/task.dart';
import 'package:client/user_info.dart';
import 'package:path_provider/path_provider.dart';

class PersistenceHelper {

  const PersistenceHelper();

  static const String metadataLocation = "metadata.json";

  // Perhaps an example where I'd like to programmatically differentiate between
  // "I can't access files" and "You don't have a username yet"
  Future<Task<UIError, UserInfo>> getUserInfo() async {
    final getDir = await _getAppDir();

    final getContents = await getDir
        .convertValue(_dirToMetadataLocation)
        .thenDoFuture(_readFromFile);

    return getContents.thenDo((raw) => UserInfo.decode(raw));
  }

  // Generalize?
  Future<Task<UIError, void>> saveUserInfo(UserInfo userInfo) async {
    final getDir = await _getAppDir();

    return await getDir
        .convertValue(_dirToMetadataLocation)
        .thenDoFuture((location) => _writeToFile(location, userInfo.encode()));
  }

  String _dirToMetadataLocation(Directory dir) {
    return "${dir.path}/$metadataLocation";
  }

  Future<Task<UIError, Directory>> _getAppDir() async {
    final Directory directory =
        await getApplicationDocumentsDirectory().catchError((e) {
      print("Error getting application directory, prob plugin not supported");
      print(e);
      // the implementation indicates it may return null, so I'll do that too :(
      return null;
    });

    if (directory == null) {
      return Fail(UIError(
          "Unable to access file system",
          "It seems path_provider is not supported on this platform, "
              "please tell us what platform you're using, and we'll investigate: "
              "github.com/lockbook -> issues"));
    }

    return Success(directory);
  }

  Future<Task<UIError, void>> _writeToFile(
      String location, String content) async {
    final file = File(location);

    try {
      file.writeAsStringSync(content);
    } catch (error) {
      return Fail(UIError("Could not write to file",
          "Error: $error while writing to $location"));
    }
    return Success(null);
  }

  Future<Task<UIError, String>> _readFromFile(String location) async {
    final file = File(location);

    try {
      return Success(file.readAsStringSync());
    } catch (error) {
      return Fail(UIError(
          "Could not read file", "Error: $error while writing to $location"));
    }
  }
}
