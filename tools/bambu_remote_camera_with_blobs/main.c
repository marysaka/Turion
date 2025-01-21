/*
 * Copyright 2025 Mary Guillemard
 * SPDX-License-Identifier: AGPL-3.0
 */

#define BAMBU_DYNAMIC

#include "BambuTunnel.h"
#include <dlfcn.h>
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>

BambuLib lib = {0};
static void *module = NULL;

void bambu_log(void *ctx, int level, tchar const *msg) {
  if (level <= 1) {
    fprintf(stdout, "[%d] %s\n", level, msg);
    lib.Bambu_FreeLogMsg(msg);
  }
}

bool handle_bambu_stream(const char *camera_url) {
  Bambu_Tunnel tunnel = NULL;
  int ret = 0;

  ret = lib.Bambu_Create(&tunnel, camera_url);
  if (ret == 0) {
    lib.Bambu_SetLogger(tunnel, bambu_log, NULL);
    ret = lib.Bambu_Open(tunnel);
    if (ret == 0)
      ret = Bambu_would_block;
  }

  while (true) {
    while (ret == Bambu_would_block) {
      usleep(100 * 1000);
      ret = lib.Bambu_StartStreamEx(tunnel, 0x3000);
    }
    fprintf(stdout, "Bambu_StartStream: %d\n", ret);

    if (ret != 0)
      break;

    Bambu_StreamInfo info;
    ret = lib.Bambu_GetStreamInfo(tunnel, 0, &info);
    fprintf(stdout, "Bambu_GetStreamInfo: %d\n", ret);

    if (ret != 0)
      break;

    fprintf(stdout, "stream format: %d\n", info.type);
    fprintf(stdout, "stream sub_type: %d\n", info.sub_type);

    while (ret == Bambu_success) {
      Bambu_Sample sample;
      ret = lib.Bambu_ReadSample(tunnel, &sample);

      while (ret == Bambu_would_block) {
        usleep(100 * 1000);
        ret = lib.Bambu_ReadSample(tunnel, &sample);
      }

      if (ret == Bambu_success) {
        fwrite(sample.buffer, 1, sample.size, stderr);
        fflush(stderr);
        continue;
      }

      fprintf(stdout, "Bambu_ReadSample ret: %d, reinit everything\n", ret);
      break;
    }
  }

  if (tunnel != NULL) {
    lib.Bambu_Close(tunnel);
    lib.Bambu_Destroy(tunnel);
  }

  return ret != 0;
}

int main(int argc, const char **argv) {
  if (argc != 3) {
    printf("Usage: %s <libBambuSource.so path> <camera_url>", argv[0]);
    return EXIT_FAILURE;
  }

  const char *bambuLibPath = argv[1];
  const char *camera_url = argv[2];

  module = dlopen(bambuLibPath, RTLD_LAZY);
  if (module == NULL) {
    fprintf(stdout, "Failed loading libBambuSource.so at path %s\n",
            bambuLibPath);
    return -1;
  }

  #define GET_FUNC(x) (lib.x = (dlsym(module, #x)))
  GET_FUNC(Bambu_Create);
  GET_FUNC(Bambu_Open);
  GET_FUNC(Bambu_StartStream);
  GET_FUNC(Bambu_StartStreamEx);
  GET_FUNC(Bambu_GetStreamCount);
  GET_FUNC(Bambu_GetStreamInfo);
  GET_FUNC(Bambu_ReadSample);
  GET_FUNC(Bambu_Close);
  GET_FUNC(Bambu_Destroy);
  GET_FUNC(Bambu_SetLogger);
  GET_FUNC(Bambu_FreeLogMsg);
  #undef GET_FUNC

  handle_bambu_stream(camera_url);

  return 0;
}
