import 'dart:io';

import 'package:client/either.dart';
import 'package:client/errors.dart';
import 'package:path_provider/path_provider.dart';

import 'main.dart';

class FileHelper {
  const FileHelper();

  Future<Either<UIError, Directory>> _getFileStoreDir() async {
    final Directory directory =
        await getApplicationDocumentsDirectory().catchError((dynamic e) {
      logger.e("Error getApplicationDocumentsDirectory(): $e");
      return null;
    });

    if (directory == null) {
      return Fail(pathProviderError());
    }

    Directory filesFolder = Directory(directory.path + "/files/");
    try {
      if (!await filesFolder.exists()) await filesFolder.create();
    } catch (error) {
      return Fail(couldNotCreateFileFolder(error));
    }
    return Success(filesFolder);
  }

  Future<Either<UIError, Empty>> writeToFile(
      String location, String content) async {
    return (await _getFileStoreDir()).flatMap((dir) {
      final file = File(dir.path + location);
      print(file);
      try {
        file.writeAsStringSync(content);
      } catch (error) {
        print(error);
        return Fail(fileWriteError(location, error));
      }
      return Success(Done);
    });
  }

  Future<Either<UIError, String>> readFromFile(String location) async {
    final getLocation = await _getFileStoreDir();

    return getLocation.flatMap((dir) {
      final file = File(dir.path + location);

      try {
        return Success(file.readAsStringSync());
      } catch (error) {
        return Fail(fileReadError(location, error));
      }
    });
  }
}
