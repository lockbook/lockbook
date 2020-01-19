import 'package:client/errors.dart';
import 'package:client/file_index_repo.dart';
import 'package:client/task.dart';
import 'package:uuid/uuid.dart';

import 'file_helper.dart';

class FileService {

  final FileIndexRepository fileRepo;
  final FileHelper fileHelper;

  const FileService(this.fileRepo, this.fileHelper);

  Future<Task<UIError, void>> createFile(String name, String content) async {
    final uuid = Uuid().v1();
    print(content);
    print(await fileHelper.writeToFile(uuid, content));
    print("here");
  }
}
