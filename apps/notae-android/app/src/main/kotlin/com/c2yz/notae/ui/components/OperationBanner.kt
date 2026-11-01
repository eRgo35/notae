package com.c2yz.notae.ui.components

import androidx.compose.animation.core.FastOutSlowInEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.ProgressBarRangeInfo
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.progressBarRangeInfo
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.c2yz.notae.R
import com.c2yz.notae.core.Action
import com.c2yz.notae.core.Banner
import com.c2yz.notae.core.Failure
import com.c2yz.notae.core.IndexFailure
import com.c2yz.notae.core.ScanSummary
import com.c2yz.notae.core.StatusMessage
import com.c2yz.notae.platform.animationsEnabled
import com.c2yz.notae.ui.theme.NotaeTheme
import com.c2yz.notae.ui.theme.Tokens
import kotlin.math.roundToInt

/**
 * 0.3.0's `OperationBanner` (library, Search and Settings): the phase text with a glyph, Cancel or Dismiss,
 * and while busy a 4 dp bar, filled to [Banner.fraction] or swept when the total is unknown. Shows nothing
 * without a message.
 */
@Composable
fun OperationBanner(banner: Banner, onAction: (Action) -> Unit, modifier: Modifier = Modifier) {
    val message = banner.message ?: return
    val c = NotaeTheme.colors
    val text = statusText(message)
    NotaeCard(modifier.fillMaxWidth()) {
        Column(Modifier.padding(start = 20.dp, top = 16.dp, end = 16.dp, bottom = 16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
            Row(horizontalArrangement = Arrangement.spacedBy(12.dp), verticalAlignment = Alignment.CenterVertically) {
                val glyph = if (banner.busy) R.drawable.ic_scan else if (banner.failed) R.drawable.ic_warning else R.drawable.ic_check
                Icon(painterResource(glyph), null, Modifier.size(24.dp), tint = if (banner.failed) c.error else c.muted)
                Text(text, Modifier.weight(1f), color = if (banner.failed) c.error else c.foreground, fontSize = 15.sp)
                if (banner.cancelEnabled) {
                    PillButton(stringResource(R.string.banner_cancel), { onAction(Action.CancelRescan) }, tonal = true)
                }
                if (banner.acknowledgeEnabled) {
                    PillButton(stringResource(R.string.banner_dismiss), { onAction(Action.AcknowledgeOperation) }, tonal = true)
                }
            }
            if (banner.busy) ProgressTrack(banner.fraction, text)
        }
    }
}

@Composable
private fun ProgressTrack(fraction: Float?, label: String) {
    val c = NotaeTheme.colors
    val shape = RoundedCornerShape(2.dp)
    BoxWithConstraints(
        Modifier
            .fillMaxWidth()
            .height(4.dp)
            .clip(shape)
            .background(c.field)
            .semantics {
                contentDescription = label
                progressBarRangeInfo = if (fraction != null) ProgressBarRangeInfo(fraction.coerceIn(0f, 1f), 0f..1f) else ProgressBarRangeInfo.Indeterminate
            },
    ) {
        if (fraction != null) {
            val filled by animateFloatAsState(fraction.coerceIn(0f, 1f), tween(Tokens.motionShortMillis), label = "banner fill")
            Box(Modifier.fillMaxHeight().fillMaxWidth(filled).clip(shape).background(c.accent))
        } else if (animationsEnabled(LocalContext.current)) {
            // A segment 30% wide crossing the empty track, as 0.3.0's 1.1 s sweep; with motion off the track stays empty.
            val travel by rememberInfiniteTransition(label = "banner sweep").animateFloat(
                initialValue = -0.3f,
                targetValue = 1f,
                animationSpec = infiniteRepeatable(tween(1100, easing = FastOutSlowInEasing), RepeatMode.Restart),
                label = "banner sweep travel",
            )
            val trackPx = constraints.maxWidth
            Box(Modifier.offset { IntOffset((trackPx * travel).roundToInt(), 0) }.width(maxWidth * 0.3f).fillMaxHeight().clip(shape).background(c.accent))
        }
    }
}

/** The banner's text (texts.rs `status_message`). */
@Composable
fun statusText(message: StatusMessage): String = when (message) {
    StatusMessage.ScanningLibrary -> stringResource(R.string.banner_scanning)
    is StatusMessage.Indexing -> pluralStringResource(R.plurals.banner_indexing, message.total.toCount(), message.total.toLong())
    is StatusMessage.ReadingText -> stringResource(R.string.banner_reading, message.completed.toLong(), message.total.toLong())
    is StatusMessage.Finished -> stringResource(R.string.banner_finished, summaryText(message.summary))
    is StatusMessage.Failed -> failureText(message.failure)
}

/** "3 added, 1 ID conflicts", or "No changes" (texts.rs `rescan_summary`). */
@Composable
private fun summaryText(summary: ScanSummary): String {
    val parts = listOf(
        R.string.summary_added to summary.added,
        R.string.summary_updated to summary.updated,
        R.string.summary_moved to summary.moved,
        R.string.summary_deleted to summary.deleted,
        R.string.summary_conflicts to summary.conflicts,
        R.string.summary_failures to summary.failures,
    ).filter { it.second != 0u }.map { (id, count) -> stringResource(id, count.toLong()) }
    return if (parts.isEmpty()) stringResource(R.string.summary_no_changes) else parts.joinToString(", ")
}

/** A failed operation or metadata edit, in the English it has always carried (texts.rs `failure`). */
@Composable
fun failureText(failure: Failure): String = when (failure) {
    is Failure.IndexQuery -> stringResource(R.string.failure_index_query, indexFailureText(failure.index))
    is Failure.IndexUpdate -> indexFailureText(failure.index)
    is Failure.InvalidNoteId -> stringResource(R.string.failure_invalid_note_id, failure.value)
    Failure.EmptyTitle -> stringResource(R.string.failure_empty_title)
    Failure.InvalidSha256Digest -> stringResource(R.string.failure_invalid_sha256)
    is Failure.InvalidRelativePath -> stringResource(R.string.failure_invalid_path, failure.path)
    Failure.MissingProvenance -> stringResource(R.string.failure_missing_provenance)
    is Failure.NoteNotFound -> stringResource(R.string.failure_note_not_found, failure.key)
    Failure.OperationAlreadyInProgress -> stringResource(R.string.failure_already_running)
    Failure.NoOperationInProgress -> stringResource(R.string.failure_nothing_running)
    Failure.RescanNotCancellable -> stringResource(R.string.failure_not_cancellable)
    Failure.WorkerStopped -> stringResource(R.string.failure_worker_stopped)
    is Failure.WorkerStart -> stringResource(R.string.failure_worker_start, failure.detail)
    is Failure.LibraryUnreadable -> stringResource(R.string.failure_library_unreadable, failure.detail)
    Failure.LibraryPermissionLost -> stringResource(R.string.failure_permission_lost)
    is Failure.EnumerationFailed -> stringResource(R.string.failure_enumeration, failure.detail)
    Failure.CancellationStateClosed -> stringResource(R.string.failure_cancellation_closed)
}

@Composable
private fun indexFailureText(failure: IndexFailure): String = when (failure) {
    is IndexFailure.Database -> stringResource(R.string.index_failure_database, failure.detail)
    is IndexFailure.DuplicateNoteId -> stringResource(R.string.index_failure_duplicate_id, failure.id.toLong())
    is IndexFailure.UnsupportedSchemaVersion ->
        stringResource(R.string.index_failure_schema_newer, failure.found.toLong(), failure.supported.toLong())
    is IndexFailure.IncompleteSchema -> stringResource(R.string.index_failure_schema_incomplete, failure.version.toLong(), failure.missing)
    is IndexFailure.UnknownNote -> stringResource(R.string.index_failure_unknown_note, failure.hash)
}

/** A count for a plural rule; one past `Int.MAX_VALUE` picks the same form as any large number. */
fun UInt.toCount(): Int = toLong().coerceAtMost(Int.MAX_VALUE.toLong()).toInt()
