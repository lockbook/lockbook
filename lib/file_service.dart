import 'package:client/errors.dart';
import 'package:client/file_description.dart';
import 'package:client/file_index_repo.dart';
import 'package:client/task.dart';

import 'file_helper.dart';

class FileService {
  final FileIndexRepository fileRepo;
  final FileHelper fileHelper;

  const FileService(this.fileRepo, this.fileHelper);

  Future<Task<UIError, Empty>> saveFile(
      String path, String name, String content) async {
    final getFile = await fileRepo.getOrCreateFileDescriptor(path, name);

    final getId = getFile.convertValue((description) => description.id);

    final saveFileContents =
        await getId.thenDoFuture((id) => fileHelper.writeToFile(id, content));

    return saveFileContents;
  }

}
