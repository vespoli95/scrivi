package main

import (
	"errors"
	"fmt"
	"os"
	"syscall"

	"fyne.io/fyne/v2"
	"fyne.io/fyne/v2/container"
	"fyne.io/fyne/v2/driver"
	"golang.org/x/sys/windows/registry"
)

const startupKey = `Software\Microsoft\Windows\CurrentVersion\Run`
const startupValue = "Scrivi"

var setWindowPos = syscall.NewLazyDLL("user32.dll").NewProc("SetWindowPos").Call

func windowOptions(w fyne.Window, db *memoryDB) fyne.CanvasObject {
	return container.NewHBox(
		newPinButton(w, func(pinned bool) error { return setWindowPinned(w, pinned) }),
		newOptionsButton(w, db, startupEnabled, setStartup),
	)
}

func setWindowPinned(w fyne.Window, pinned bool) error {
	native, ok := w.(driver.NativeWindow)
	if !ok {
		return errors.New("pin window: native window is unavailable")
	}
	var result error
	native.RunNative(func(context any) {
		window, ok := context.(driver.WindowsWindowContext)
		if !ok || window.HWND == 0 {
			result = errors.New("pin window: native window is unavailable")
			return
		}
		insertAfter := ^uintptr(1) // HWND_NOTOPMOST
		if pinned {
			insertAfter = ^uintptr(0) // HWND_TOPMOST
		}
		// Preserve the window's position, size, and activation.
		success, _, err := setWindowPos(window.HWND, insertAfter, 0, 0, 0, 0, 0x0001|0x0002|0x0010)
		if success == 0 {
			if err == syscall.Errno(0) {
				result = errors.New("pin window: SetWindowPos failed")
			} else {
				result = fmt.Errorf("pin window: %w", err)
			}
		}
	})
	return result
}

func startupEnabled() (bool, error) {
	key, err := registry.OpenKey(registry.CURRENT_USER, startupKey, registry.QUERY_VALUE)
	if errors.Is(err, registry.ErrNotExist) {
		return false, nil
	}
	if err != nil {
		return false, fmt.Errorf("read startup setting: %w", err)
	}
	defer key.Close()
	_, _, err = key.GetStringValue(startupValue)
	if errors.Is(err, registry.ErrNotExist) {
		return false, nil
	}
	if err != nil {
		return false, fmt.Errorf("read startup setting: %w", err)
	}
	return true, nil
}

func setStartup(enabled bool) error {
	if !enabled {
		key, err := registry.OpenKey(registry.CURRENT_USER, startupKey, registry.SET_VALUE)
		if errors.Is(err, registry.ErrNotExist) {
			return nil
		}
		if err != nil {
			return fmt.Errorf("disable startup: %w", err)
		}
		defer key.Close()
		if err := key.DeleteValue(startupValue); err != nil && !errors.Is(err, registry.ErrNotExist) {
			return fmt.Errorf("disable startup: %w", err)
		}
		return nil
	}
	executable, err := os.Executable()
	if err != nil {
		return fmt.Errorf("find executable for startup: %w", err)
	}
	key, _, err := registry.CreateKey(registry.CURRENT_USER, startupKey, registry.SET_VALUE)
	if err != nil {
		return fmt.Errorf("enable startup: %w", err)
	}
	defer key.Close()
	if err := key.SetStringValue(startupValue, `"`+executable+`"`); err != nil {
		return fmt.Errorf("enable startup: %w", err)
	}
	return nil
}
