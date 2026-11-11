package main

import (
	"fyne.io/fyne/v2"
	"fyne.io/fyne/v2/container"
	"fyne.io/fyne/v2/dialog"
	"fyne.io/fyne/v2/theme"
)

func newOptionsButton(w fyne.Window, setPinned func(bool) error, startupEnabled func() (bool, error), setStartup func(bool) error) *pointerButton {
	pinned := false
	return newPointerButton("Settings", theme.SettingsIcon(), func() {
		startup, startupErr := startupEnabled()
		pinCheck := newPointerCheck()
		pinCheck.Text = "Keep window on top"
		pinCheck.SetChecked(pinned)
		var changePin func(bool)
		changePin = func(value bool) {
			if err := setPinned(value); err != nil {
				pinCheck.OnChanged = nil
				pinCheck.SetChecked(pinned)
				pinCheck.OnChanged = changePin
				showError(err, w)
				return
			}
			pinned = value
		}
		pinCheck.OnChanged = changePin
		startupCheck := newPointerCheck()
		startupCheck.Text = "Start Scrivi when I sign in"
		startupCheck.SetChecked(startup)
		if startupErr != nil {
			startupCheck.Disable()
		}
		var changeStartup func(bool)
		changeStartup = func(value bool) {
			if err := setStartup(value); err != nil {
				startupCheck.OnChanged = nil
				startupCheck.SetChecked(startup)
				startupCheck.OnChanged = changeStartup
				showError(err, w)
				return
			}
			startup = value
		}
		startupCheck.OnChanged = changeStartup
		d := dialog.NewCustomWithoutButtons("Settings", container.NewVBox(pinCheck, startupCheck), w)
		d.SetButtons([]fyne.CanvasObject{newPointerButton("Close", nil, d.Hide)})
		d.Show()
		if startupErr != nil {
			showError(startupErr, w)
		}
	})
}
