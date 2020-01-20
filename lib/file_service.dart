import 'package:client/errors.dart';
import 'package:client/file_index_repo.dart';
import 'package:client/either.dart';

import 'file_helper.dart';

class FileService {
  final FileIndexRepository fileRepo;
  final FileHelper fileHelper;

  const FileService(this.fileRepo, this.fileHelper);

  Future<Either<UIError, Empty>> saveFile(
      String path, String name, String content) async {
    final getFile = await fileRepo.getOrCreateFileDescriptor(path, name);

    final Either<UIError, String> getId =
        getFile.map((description) => description.id);

    final Either<UIError, Empty> saveFileContents = await getId
        .flatMapFut((String id) => fileHelper.writeToFile(id, content));

    return saveFileContents;
  }
}
