@echo off
set "JAVA_HOME=C:\Program Files\Huawei\DevEco Studio\jbr"
set "NODE_HOME=C:\Program Files\Huawei\DevEco Studio\tools\node"
set "DEVECO_SDK_HOME=C:\Program Files\Huawei\DevEco Studio\sdk"
set "OHOS_BASE_SDK_HOME=C:\Program Files\Huawei\DevEco Studio\sdk\default\openharmony"
set "PATH=%NODE_HOME%;%JAVA_HOME%\bin;C:\Program Files\Huawei\DevEco Studio\tools\ohpm\bin;C:\Program Files\Huawei\DevEco Studio\tools\hvigor\bin;%PATH%"
cd /d C:\Users\Administrator\Projects\HarmonyDesk\ohos
call "C:\Program Files\Huawei\DevEco Studio\tools\hvigor\bin\hvigorw.bat" --mode module -p product=default -p module=entry@default -p buildMode=debug assembleHap --no-daemon
exit /b %ERRORLEVEL%
