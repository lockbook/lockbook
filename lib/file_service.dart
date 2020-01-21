import 'package:client/either.dart';
import 'package:client/encryption_helper.dart';
import 'package:client/errors.dart';
import 'package:client/file_index_repo.dart';
import 'package:client/last_update_repo.dart';
import 'package:client/network_helper.dart';
import 'package:client/user_repository.dart';

import 'file_helper.dart';

class FileService {
  final UserRepository userRepository;
  final FileIndexRepository fileRepo;
  final FileHelper fileHelper;
  final EncryptionHelper encryptionHelper;
  final NetworkHelper networkHelper;
  final LastUpdateRepository lastUpdateRepository;

  const FileService(this.fileRepo, this.fileHelper, this.networkHelper,
      this.encryptionHelper, this.userRepository, this.lastUpdateRepository);

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

  Future<Either<UIError, Empty>> sync(
      dynamic Function(double status) updateStatus) async {
    final getUserInfo = await userRepository.getUserInfo();
    if (!getUserInfo.isSuccessful()) return getUserInfo.map((_) => Done);
    final userInfo = getUserInfo.getValueUnsafely();
    print("userInfo: $userInfo");

    final getLastUpdated = await lastUpdateRepository.getLastUpdated();
    updateStatus(0.05);

    print("lastUpdated: ${getLastUpdated.getValueUnsafely()}");

    final getListOfChangedFiles =
        await getLastUpdated.flatMapFut(networkHelper.getFilesChangedSince);
    updateStatus(0.1);

    final getDecryptAndSaveEachFile =
        await getListOfChangedFiles.flatMapFut((ids) async {
      for (int i = 0; i < ids.length; i++) {
        final progress = ((i / ids.length) * 0.85) + 0.1;
        print("progress $progress");

        String id = ids[i];
        final encryptedBody = await networkHelper.getFile(id);

        print("encrypted: ${encryptedBody.getValueUnsafely()}");

        final decrypt = encryptedBody.flatMap(
            (encrypted) => encryptionHelper.decrypt(userInfo, encrypted));

        print("decypted: ${decrypt.getValueUnsafely()}");

        final saveLocally = decrypt
            .flatMapFut((decrypted) => fileHelper.writeToFile(id, decrypted));

        updateStatus(progress);
      }

      return Success(Done);
    });

    updateStatus(1);
    return getDecryptAndSaveEachFile;
  }
}
