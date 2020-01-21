import 'package:client/either.dart';
import 'package:client/encryption_helper.dart';
import 'package:client/errors.dart';
import 'package:client/file_index_repo.dart';
import 'package:client/network_helper.dart';
import 'package:client/user_repository.dart';

import 'file_helper.dart';

class FileService {
  final UserRepository userRepository;
  final FileIndexRepository fileRepo;
  final FileHelper fileHelper;
  final EncryptionHelper encryptionHelper;
  final NetworkHelper networkHelper;

  const FileService(this.fileRepo, this.fileHelper, this.networkHelper,
      this.encryptionHelper, this.userRepository);

  Future<Either<UIError, Empty>> saveFile(
      String path, String name, String content) async {
    final getFile = await fileRepo.getOrCreateFileDescriptor(path, name);
    final getId = getFile.map((description) => description.id);

    final saveFileContents =
        await getId.flatMapFut((id) => fileHelper.writeToFile(id, content));

    if (!saveFileContents.isSuccessful()) return saveFileContents;
    final fileDescriptor = getFile.getValueUnsafely();

    final maybeUserInfo = await userRepository.getUserInfo();
    if (!maybeUserInfo.isSuccessful()) return maybeUserInfo.map((_) => Done);

    final userInfo = maybeUserInfo.getValueUnsafely();
    final encryptedContent = encryptionHelper.encrypt(userInfo, content);

    return networkHelper.uploadFile(userInfo, fileDescriptor, encryptedContent);
  }
}
