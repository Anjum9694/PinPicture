{
  TODO:
  - Effects consume a lot of memory, so it is disabled
}
unit main;

interface

uses
  System.SysUtils, System.Types, System.UITypes, System.Classes, System.Variants,
  FMX.Types, FMX.Controls, FMX.Forms, FMX.Graphics, FMX.Dialogs,
  // DragDrop, DropTarget and DragDropText were dropped from this unit: the
  // form uses no drag/drop component and no code from them.
  FMX.Layouts, FMX.ExtCtrls, FMX.Objects,
  System.Math.Vectors, FMX.Controls3D, FMX.Layers3D, Skia, Skia.FMX,
  FMX.Viewport3D, FMX.Controls.Presentation, FMX.StdCtrls, FMX.Colors,
  FMX.Effects, FMX.Filter.Effects, FMX.ImageLayout,
  Winapi.Windows, Winapi.ShellAPI;

type
  TForm2 = class(TForm)
    Image1: TImage;
    Viewport3D1: TViewport3D;
    Layer3D1: TLayer3D;
    StyleBook1: TStyleBook;
    raDragArea: TRectangle;
    SpeedButton1: TSpeedButton;
    raSettings: TRectangle;
    raResizeArea: TRectangle;
    AlphaTrackBar1: TAlphaTrackBar;
    BWTrackBar1: TBWTrackBar;
    HueTrackBar1: THueTrackBar;
    HueAdjustEffect1: THueAdjustEffect;
    ToonEffect1: TToonEffect;
    BlurEffect1: TBlurEffect;
    ContrastEffect1: TContrastEffect;
    BWTrackBar2: TBWTrackBar;
    tbRAngleX: TTrackBar;
    tbRAngleY: TTrackBar;
    tbRAngleZ: TTrackBar;
    SpeedButton2: TSpeedButton;
    SpeedButton3: TSpeedButton;
    SpeedButton4: TSpeedButton;
    SpeedButton5: TSpeedButton;
    OpenDialog1: TOpenDialog;
    PaintBox1: TPaintBox;
    ImageLayout1: TImageLayout;
    SpeedButton6: TSpeedButton;
    SpeedButton7: TSpeedButton;
    SpeedButton8: TSpeedButton;
    ChkAutoStart: TCheckBox;
    CalloutRectangle1: TCalloutRectangle;
    Label1: TLabel;
    Label2: TLabel;
    procedure Image1MouseDown(Sender: TObject; Button: TMouseButton;
      Shift: TShiftState; X, Y: Single);
    procedure SkSvg1MouseDown(Sender: TObject; Button: TMouseButton;
      Shift: TShiftState; X, Y: Single);
    procedure FormCreate(Sender: TObject);
    procedure SkAnimatedImage1MouseDown(Sender: TObject; Button: TMouseButton;
      Shift: TShiftState; X, Y: Single);
    procedure raDragAreaMouseDown(Sender: TObject; Button: TMouseButton;
      Shift: TShiftState; X, Y: Single);
    procedure raResizeAreaMouseDown(Sender: TObject; Button: TMouseButton;
      Shift: TShiftState; X, Y: Single);
    procedure HueTrackBar1Change(Sender: TObject);
    procedure AlphaTrackBar1Change(Sender: TObject);
    procedure BWTrackBar1Change(Sender: TObject);
    procedure BWTrackBar2Change(Sender: TObject);
    procedure BWTrackBar1DblClick(Sender: TObject);
    procedure HueTrackBar1DblClick(Sender: TObject);
    procedure BWTrackBar2DblClick(Sender: TObject);
    procedure AlphaTrackBar1DblClick(Sender: TObject);
    procedure tbRAngleXChange(Sender: TObject);
    procedure tbRAngleYChange(Sender: TObject);
    procedure tbRAngleZChange(Sender: TObject);
    procedure SpeedButton4Click(Sender: TObject);
    procedure SpeedButton3Click(Sender: TObject);
    procedure SpeedButton2Click(Sender: TObject);
    procedure SpeedButton5Click(Sender: TObject);
    procedure SpeedButton1Click(Sender: TObject);
    procedure raDragAreaMouseEnter(Sender: TObject);
    procedure raDragAreaMouseLeave(Sender: TObject);
    procedure Image1DblClick(Sender: TObject);
    procedure Image1MouseWheel(Sender: TObject; Shift: TShiftState;
      WheelDelta: Integer; var Handled: Boolean);
    procedure ImageLayout1MouseDown(Sender: TObject; Button: TMouseButton;
      Shift: TShiftState; X, Y: Single);
    procedure ImageLayout1DblClick(Sender: TObject);
    procedure FormActivate(Sender: TObject);
    procedure SpeedButton6Click(Sender: TObject);
    procedure tbRAngleXDblClick(Sender: TObject);
    procedure tbRAngleYDblClick(Sender: TObject);
    procedure tbRAngleZDblClick(Sender: TObject);
    procedure SpeedButton7Click(Sender: TObject);
    procedure SpeedButton8Click(Sender: TObject);
    procedure ChkAutoStartChange(Sender: TObject);
    procedure FormShow(Sender: TObject);
  private
    { Private declarations }
    PrevWheelDelta: Extended;
    FSettingUp: Boolean;
    FTrayActive: Boolean;
    FTrayWnd: HWND;
    FTrayIcon: HICON;
    FTrayNid: TNotifyIconData;
    procedure PinFormClose(Sender: TObject; var Action: TCloseAction);
    procedure FormDestroy(Sender: TObject);
    procedure InitTrayIcon;
    procedure RemoveTrayIcon;
    procedure SetPinsVisible(const AVisible: Boolean);
    procedure TogglePinsFromTray;
    procedure ShowTrayMenu;
  public
    { Public declarations }
  end;

var
  Form2: TForm2;

implementation

{$R *.fmx}

uses
  FMX.Platform.Win, Winapi.Messages, Winapi.Windows, Winapi.ShellAPI,
  System.Win.Registry, System.Generics.Collections;

const
  // tray icon plumbing
  WM_TRAYICON     = WM_APP + 1;  // callback message from Shell_NotifyIcon
  TRAY_ICON_ID    = 1;           // uID of our tray icon
  IDM_TRAY_TOGGLE = 1;           // tray popup menu ids
  IDM_TRAY_EXIT   = 2;
  // Windows login auto-start (HKCU Run key)
  AUTOSTART_KEY   = 'Software\Microsoft\Windows\CurrentVersion\Run';
  AUTOSTART_NAME  = 'PinPicture';

var
  GPinForms: TList<TForm2>;   // every live pin window (main + extra pictures)
  GTrayForm: TForm2;          // pin window that currently owns the tray icon
  GTrayPrevProc: Pointer;     // original window procedure of the tray owner

function AutoStartEnabled: Boolean;
var
  Reg: TRegistry;
begin
  Result := False;
  Reg := TRegistry.Create(KEY_READ);
  try
    Reg.RootKey := HKEY_CURRENT_USER;
    try
      if Reg.OpenKeyReadOnly(AUTOSTART_KEY) then
        try
          Result := Reg.ValueExists(AUTOSTART_NAME);
        finally
          Reg.CloseKey;
        end;
    except
      // unreadable Run key: report "disabled"
    end;
  finally
    Reg.Free;
  end;
end;

procedure SetAutoStartEnabled(const AEnable: Boolean);
var
  Reg: TRegistry;
begin
  Reg := TRegistry.Create(KEY_WRITE);
  try
    Reg.RootKey := HKEY_CURRENT_USER;
    try
      if Reg.OpenKey(AUTOSTART_KEY, True) then
        try
          if AEnable then
            Reg.WriteString(AUTOSTART_NAME, QuotedStr(ParamStr(0)))
          else if Reg.ValueExists(AUTOSTART_NAME) then
            Reg.DeleteValue(AUTOSTART_NAME);
        finally
          Reg.CloseKey;
        end;
    except
      on E: Exception do
        ShowMessage('Could not update the Windows auto-start setting: ' + E.Message);
    end;
  finally
    Reg.Free;
  end;
end;

// Subclasses the owner form's Win32 window so the Shell_NotifyIcon callback
// message reaches us (FMX itself does not expose window messages).
function PinPictureTrayWndProc(HWnd: HWND; Msg: UINT; WParam: WPARAM;
  LParam: LPARAM): LRESULT; stdcall;
var
  Event: Word;
begin
  if (Msg = WM_TRAYICON) and Assigned(GTrayForm) then
  begin
    Event := LoWord(DWord(LParam));
    if Event = WM_LBUTTONDBLCLK then
      GTrayForm.TogglePinsFromTray
    else if Event = WM_RBUTTONUP then
      GTrayForm.ShowTrayMenu;
    Result := 0;
    Exit;
  end;
  Result := CallWindowProc(GTrayPrevProc, HWnd, Msg, WParam, LParam);
end;

procedure TForm2.AlphaTrackBar1Change(Sender: TObject);
begin
//  Image1.Opacity := AlphaTrackBar1.Value/100;
  ImageLayout1.Opacity := AlphaTrackBar1.Value/100;
end;

procedure TForm2.AlphaTrackBar1DblClick(Sender: TObject);
begin
  AlphaTrackBar1.Value := 100;
end;

procedure TForm2.BWTrackBar1Change(Sender: TObject);
begin
  ContrastEffect1.Contrast := BWTrackBar1.Value / 100;
end;

procedure TForm2.BWTrackBar1DblClick(Sender: TObject);
begin
  BWTrackBar1.Value := 100;
end;

procedure TForm2.BWTrackBar2Change(Sender: TObject);
begin
  ContrastEffect1.Brightness := BWTrackBar2.Value / 100;
end;

procedure TForm2.BWTrackBar2DblClick(Sender: TObject);
begin
  BWTrackBar2.Value := 0;
end;

procedure TForm2.ChkAutoStartChange(Sender: TObject);
begin
  if FSettingUp then
    Exit;
  SetAutoStartEnabled(ChkAutoStart.IsChecked);
end;

procedure TForm2.FormActivate(Sender: TObject);
begin
  SpeedButton1.Opacity := 1;
  SpeedButton2.Opacity := 1;
  SpeedButton3.Opacity := 1;
  SpeedButton4.Opacity := 1;
  SpeedButton5.Opacity := 1;
  SpeedButton6.Opacity := 1;
  AlphaTrackBar1.Opacity := 1;
  raSettings.Opacity := 1;
  tbRAngleX.Opacity := 1;
  tbRAngleY.Opacity := 1;
  tbRAngleZ.Opacity := 1;
  raResizeArea.Opacity := 1;
  raDragArea.Opacity := 1;
  SetWindowLong(FmxHandleToHWND(Handle), GWL_EXSTYLE, GetWindowLong(FmxHandleToHWND(Handle), GWL_EXSTYLE) and not WS_EX_TRANSPARENT);

end;

procedure TForm2.FormCreate(Sender: TObject);
begin
//  FormStyle := TFormStyle.StayOnTop;
  if GPinForms = nil then
    GPinForms := TList<TForm2>.Create;
  GPinForms.Add(Self);
  OnClose := PinFormClose;
  OnDestroy := FormDestroy;
  FSettingUp := True;
  try
    OpenDialog1.Options := OpenDialog1.Options + [ofAllowMultiSelect];
    OpenDialog1.Filter := 'Picture files|*.png;*.jpg;*.jpeg;*.gif;*.bmp;*.tif;*.tiff;*.webp|All files|*.*';
    ChkAutoStart.IsChecked := AutoStartEnabled;
  finally
    FSettingUp := False;
  end;
end;

procedure TForm2.FormDestroy(Sender: TObject);
var
  I: Integer;
begin
  if Self = GTrayForm then
    RemoveTrayIcon;
  if GPinForms <> nil then
    GPinForms.Remove(Self);
  // When the tray owner dies but the app lives on (main form was closed while
  // other pictures were still pinned), hand the tray icon to a surviving pin.
  if (GTrayForm = nil) and (GPinForms <> nil) and (GPinForms.Count > 0) then
    for I := 0 to GPinForms.Count - 1 do
      if not GPinForms[I].Released then
      begin
        GPinForms[I].InitTrayIcon;
        Break;
      end;
end;

procedure TForm2.FormShow(Sender: TObject);
begin
  InitTrayIcon;
end;

procedure TForm2.InitTrayIcon;
var
  ExeDir: string;
begin
  if FTrayActive or (GTrayForm <> nil) then
    Exit;
  FTrayWnd := FmxHandleToHWND(Handle);
  if FTrayWnd = 0 then
    Exit;
  // prefer the shipped icons when they sit next to the exe, else app icon
  ExeDir := ExtractFilePath(ParamStr(0));
  FTrayIcon := 0;
  if FileExists(ExeDir + 'ico_white.ico') then
    FTrayIcon := ExtractIcon(HInstance, PChar(ExeDir + 'ico_white.ico'), 0);
  if (FTrayIcon = 0) and FileExists(ExeDir + 'ico_dark.ico') then
    FTrayIcon := ExtractIcon(HInstance, PChar(ExeDir + 'ico_dark.ico'), 0);
  if FTrayIcon = 0 then
    FTrayIcon := LoadIcon(HInstance, PChar('MAINICON'));
  if FTrayIcon = 0 then
    FTrayIcon := LoadIcon(0, IDI_APPLICATION);
  FillChar(FTrayNid, SizeOf(FTrayNid), 0);
  FTrayNid.cbSize := SizeOf(FTrayNid);
  FTrayNid.Wnd := FTrayWnd;
  FTrayNid.uID := TRAY_ICON_ID;
  FTrayNid.uFlags := NIF_ICON or NIF_MESSAGE or NIF_TIP;
  FTrayNid.uCallbackMessage := WM_TRAYICON;
  FTrayNid.hIcon := FTrayIcon;
  FTrayNid.szTip := 'PinPicture';
  GTrayForm := Self;
  GTrayPrevProc := Pointer(GetWindowLongPtr(FTrayWnd, GWLP_WNDPROC));
  SetWindowLongPtr(FTrayWnd, GWLP_WNDPROC, NativeInt(@PinPictureTrayWndProc));
  Shell_NotifyIcon(NIM_ADD, @FTrayNid);
  FTrayActive := True;
end;

procedure TForm2.RemoveTrayIcon;
begin
  if FTrayActive then
  begin
    Shell_NotifyIcon(NIM_DELETE, @FTrayNid);
    FTrayActive := False;
  end;
  if GTrayForm = Self then
  begin
    if (FTrayWnd <> 0) and Assigned(GTrayPrevProc) and IsWindow(FTrayWnd) then
      SetWindowLongPtr(FTrayWnd, GWLP_WNDPROC, NativeInt(GTrayPrevProc));
    GTrayForm := nil;
  end;
  if FTrayIcon <> 0 then
  begin
    DestroyIcon(FTrayIcon);
    FTrayIcon := 0;
  end;
end;

procedure TForm2.SetPinsVisible(const AVisible: Boolean);
var
  I: Integer;
begin
  if GPinForms = nil then
    Exit;
  for I := 0 to GPinForms.Count - 1 do
    if not GPinForms[I].Released then
      GPinForms[I].Visible := AVisible;
end;

procedure TForm2.TogglePinsFromTray;
var
  I: Integer;
  AnyVisible: Boolean;
begin
  AnyVisible := False;
  if GPinForms <> nil then
    for I := 0 to GPinForms.Count - 1 do
      if not GPinForms[I].Released then
        if GPinForms[I].Visible then
        begin
          AnyVisible := True;
          Break;
        end;
  SetPinsVisible(not AnyVisible);
end;

procedure TForm2.ShowTrayMenu;
var
  Menu: HMENU;
  Cmd: Integer;
  MP: LongWord;
  P: TPoint;
begin
  Menu := CreatePopupMenu;
  if Menu = 0 then
    Exit;
  try
    AppendMenu(Menu, MF_STRING, IDM_TRAY_TOGGLE, 'Show / Hide Pins');
    AppendMenu(Menu, MF_STRING, IDM_TRAY_EXIT, 'Exit PinPicture');
    MP := LongWord(GetMessagePos);
    P.X := SmallInt(LoWord(MP));
    P.Y := SmallInt(HiWord(MP));
    SetForegroundWindow(FTrayWnd);
    // TPM_RETURNCMD: return value is the picked menu id
    Cmd := Integer(TrackPopupMenu(Menu,
      TPM_LEFTALIGN or TPM_BOTTOMALIGN or TPM_RIGHTBUTTON or TPM_RETURNCMD,
      P.X, P.Y, 0, FTrayWnd, nil));
    if Cmd = IDM_TRAY_TOGGLE then
      TogglePinsFromTray
    else if Cmd = IDM_TRAY_EXIT then
      Close;
  finally
    DestroyMenu(Menu);
  end;
end;

procedure TForm2.PinFormClose(Sender: TObject; var Action: TCloseAction);
var
  I: Integer;
begin
  // FireMonkey default: closing the main form terminates the whole app and
  // closing a secondary form only hides it. Make every pin window close
  // individually: free it, and when the main form is the one that is closed
  // while other pins are still open, promote a surviving pin to main form so
  // the application keeps running.
  if Self = Application.MainForm then
  begin
    if GPinForms <> nil then
      for I := 0 to GPinForms.Count - 1 do
        if (GPinForms[I] <> Self) and not GPinForms[I].Released then
        begin
          Application.MainForm := GPinForms[I];
          Action := TCloseAction.caFree;
          Break;
        end;
    // no other pins: leave Action = caHide, Close() will call
    // Application.Terminate because this is still the main form
  end
  else
    Action := TCloseAction.caFree;
end;

procedure TForm2.HueTrackBar1Change(Sender: TObject);
begin
  HueAdjustEffect1.Hue := HueTrackBar1.Value / 100;
end;

procedure TForm2.HueTrackBar1DblClick(Sender: TObject);
begin
  HueTrackBar1.Value := 0;
end;

procedure TForm2.Image1DblClick(Sender: TObject);
begin
  raDragArea.Visible := not raDragArea.Visible;
  raResizeArea.Visible := not raResizeArea.Visible;
end;

procedure TForm2.Image1MouseDown(Sender: TObject; Button: TMouseButton;
  Shift: TShiftState; X, Y: Single);
begin
  ReleaseCapture;
  PostMessage(FmxHandleToHWND(Handle), WM_SYSCOMMAND, $F012, 0);
end;

procedure TForm2.Image1MouseWheel(Sender: TObject; Shift: TShiftState;
  WheelDelta: Integer; var Handled: Boolean);
var
  zoom_center: TPointF;
  new_scale, old_scale: Single;
begin

{  zoom_center := Image1.Position.Point + PointF(Image1.Width / 2, Image1.Height / 2);

  if WheelDelta >= 0 then
  begin
    Image1.Width := Image1.Width + (WheelDelta);
    Image1.Height := Image1.Height + (WheelDelta);
  end
  else
  begin
    Image1.Width := Image1.Width - (WheelDelta);
    Image1.Height := Image1.Height - (WheelDelta);

  end;


  Image1.Position.X := zoom_center.X - Image1.Width / 2;
  Image1.Position.Y := zoom_center.Y - Image1.Height / 2;
  Exit;}
 if ssCtrl in Shift then
 begin
  zoom_center := ScreenToClient(Screen.MousePos) -  PaintBox1.Position.Point;

  old_scale := Image1.Scale.X;
  if WheelDelta >= 0 then
    new_scale := old_scale * (1 + (WheelDelta / 120) /5)
  else
    new_scale := old_scale / (1 - (WheelDelta / 120) /5);

  Image1.Scale.X := new_scale;
  Image1.Scale.Y := new_scale;
  Image1.Position.Point := zoom_center - (new_scale * (zoom_center - Image1.Position.Point) / old_scale);

 end;
end;

procedure TForm2.ImageLayout1DblClick(Sender: TObject);
begin
  raDragArea.Visible := not raDragArea.Visible;
  raResizeArea.Visible := not raResizeArea.Visible;
end;

procedure TForm2.ImageLayout1MouseDown(Sender: TObject; Button: TMouseButton;
  Shift: TShiftState; X, Y: Single);
begin
  if ssCtrl in Shift then
  begin
    ReleaseCapture;
    PostMessage(FmxHandleToHWND(Handle), WM_SYSCOMMAND, $F012, 0);
  end;
end;

procedure TForm2.raDragAreaMouseDown(Sender: TObject; Button: TMouseButton;
  Shift: TShiftState; X, Y: Single);
begin
  ReleaseCapture;
  PostMessage(FmxHandleToHWND(Handle), WM_SYSCOMMAND, $F012, 0);
end;

procedure TForm2.raDragAreaMouseEnter(Sender: TObject);
begin
//  raDragArea.Fill.Color := $64E0E0E0;
raDragArea.Fill.Color := $64181818;
end;

procedure TForm2.raDragAreaMouseLeave(Sender: TObject);
begin
  raDragArea.Fill.Color := $51181818;
end;

procedure TForm2.raResizeAreaMouseDown(Sender: TObject; Button: TMouseButton;
  Shift: TShiftState; X, Y: Single);
begin
  ReleaseCapture;
  PostMessage(FmxHandleToHWND(Handle), WM_SYSCOMMAND, SC_SIZE or WMSZ_BOTTOMRIGHT, 0);
end;

procedure TForm2.SkAnimatedImage1MouseDown(Sender: TObject;
  Button: TMouseButton; Shift: TShiftState; X, Y: Single);
begin
  ReleaseCapture;
  PostMessage(FmxHandleToHWND(Handle), WM_SYSCOMMAND, $F012, 0);
end;

procedure TForm2.SkSvg1MouseDown(Sender: TObject; Button: TMouseButton;
  Shift: TShiftState; X, Y: Single);
begin
  ReleaseCapture;
  PostMessage(FmxHandleToHWND(Handle), WM_SYSCOMMAND, $F012, 0);
end;

procedure TForm2.SpeedButton1Click(Sender: TObject);
begin
  if FormStyle = TFormStyle.Normal then
  begin
    FormStyle := TFormStyle.StayOnTop;
    SpeedButton1.Text := ''
  end
  else
  begin
    FormStyle := TFormStyle.Normal;
    SpeedButton1.Text := '';
  end;
end;

procedure TForm2.SpeedButton2Click(Sender: TObject);
begin
  raSettings.Visible := not raSettings.Visible;
end;

procedure TForm2.SpeedButton3Click(Sender: TObject);
begin
  // PinFormClose decides hide/free/terminate for this window
  Close;
end;

procedure TForm2.SpeedButton4Click(Sender: TObject);
begin
  if raDragArea.Align = TAlignLayout.Top then
    raDragArea.Align := TAlignLayout.Bottom
  else
    raDragArea.Align := TAlignLayout.Top;
end;

procedure TForm2.SpeedButton5Click(Sender: TObject);
var
  I: Integer;
  NewForm: TForm2;
begin
  if not OpenDialog1.Execute then
    Exit;
  // first selected picture replaces the current window's picture...
  if OpenDialog1.Files.Count > 0 then
    ImageLayout1.Image.LoadFromFile(OpenDialog1.Files[0])
  else
    ImageLayout1.Image.LoadFromFile(OpenDialog1.FileName);
  // ...every extra selected picture opens its own pin window
  for I := 1 to OpenDialog1.Files.Count - 1 do
  begin
    NewForm := TForm2.Create(Application);
    NewForm.ImageLayout1.Image.LoadFromFile(OpenDialog1.Files[I]);
    NewForm.Position.X := Position.X + 32 * I;
    NewForm.Position.Y := Position.Y + 32 * I;
    NewForm.Show;
  end;
end;

procedure TForm2.SpeedButton6Click(Sender: TObject);
begin
// transparent all
  SpeedButton1.Opacity := 0;
  SpeedButton2.Opacity := 0;
  SpeedButton3.Opacity := 0;
  SpeedButton4.Opacity := 0;
  SpeedButton5.Opacity := 0;
  SpeedButton6.Opacity := 0;
  AlphaTrackBar1.Opacity := 0;
  raSettings.Opacity := 0;
  tbRAngleX.Opacity := 0;
  tbRAngleY.Opacity := 0;
  tbRAngleZ.Opacity := 0;
  raResizeArea.Opacity := 0;
  raDragArea.Opacity := 0;
  SetWindowLong(FmxHandleToHWND(Handle), GWL_EXSTYLE, GetWindowLong(FmxHandleToHWND(Handle), GWL_EXSTYLE) or WS_EX_TRANSPARENT);
end;

procedure TForm2.SpeedButton7Click(Sender: TObject);
begin
  CalloutRectangle1.Visible := not CalloutRectangle1.Visible;
end;

procedure TForm2.SpeedButton8Click(Sender: TObject);
begin
  // minimize all pinned pictures to the tray icon
  SetPinsVisible(False);
end;

procedure TForm2.tbRAngleXChange(Sender: TObject);
begin
  Layer3D1.RotationAngle.X := tbRAngleX.Value;
end;

procedure TForm2.tbRAngleXDblClick(Sender: TObject);
begin
  tbRAngleX.Value := 340;
end;

procedure TForm2.tbRAngleYChange(Sender: TObject);
begin
  Layer3D1.RotationAngle.Y := tbRAngleY.Value;
end;

procedure TForm2.tbRAngleYDblClick(Sender: TObject);
begin
  tbRAngleY.Value := 0;
end;

procedure TForm2.tbRAngleZChange(Sender: TObject);
begin
  Layer3D1.RotationAngle.Z := tbRAngleZ.Value;
end;

procedure TForm2.tbRAngleZDblClick(Sender: TObject);
begin
  tbRAngleZ.Value := 0;
end;

end.
