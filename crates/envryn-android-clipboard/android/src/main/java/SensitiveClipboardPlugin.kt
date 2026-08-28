package dev.envryn.clipboard

import android.app.Activity
import android.content.ClipData
import android.content.ClipDescription
import android.content.ClipboardManager
import android.content.Context
import android.os.Build
import android.os.PersistableBundle
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin

@InvokeArg
class WriteSensitiveTextArgs {
  lateinit var text: String
}

@TauriPlugin
class SensitiveClipboardPlugin(private val activity: Activity) : Plugin(activity) {
  private val manager =
    activity.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager

  @Command
  fun writeSensitiveText(invoke: Invoke) {
    val args = invoke.parseArgs(WriteSensitiveTextArgs::class.java)
    val clip = ClipData.newPlainText("Envryn secret", args.text)

    // Android 13 hides previews for clips with this standard flag. The
    // literal key is intentionally used so the same protected metadata is
    // also attached on older supported API levels without an SDK guard.
    val extras = PersistableBundle()
    extras.putBoolean("android.content.extra.IS_SENSITIVE", true)
    clip.description.extras = extras

    manager.setPrimaryClip(clip)
    invoke.resolve()
  }

  @Command
  fun readText(invoke: Invoke) {
    val description = manager.primaryClipDescription
    if (!manager.hasPrimaryClip() ||
      description?.hasMimeType(ClipDescription.MIMETYPE_TEXT_PLAIN) != true) {
      invoke.reject("Clipboard does not contain plain text")
      return
    }

    val text = manager.primaryClip?.getItemAt(0)?.coerceToText(activity)?.toString()
    if (text == null) {
      invoke.reject("Clipboard is empty")
      return
    }
    val response = JSObject()
    response.put("text", text)
    invoke.resolve(response)
  }

  @Command
  fun clear(invoke: Invoke) {
    if (manager.hasPrimaryClip()) {
      if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
        manager.clearPrimaryClip()
      } else {
        manager.setPrimaryClip(ClipData.newPlainText("", ""))
      }
    }
    invoke.resolve()
  }
}
