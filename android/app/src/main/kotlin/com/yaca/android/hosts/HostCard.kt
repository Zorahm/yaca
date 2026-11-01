@file:OptIn(ExperimentalMaterial3Api::class)

package com.yaca.android.hosts

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Computer
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.yaca.android.R
import com.yaca.android.pairing.refusalMessage
import com.yaca.android.ui.common.ConnectionDot
import com.yaca.android.ui.common.ForgetMenu
import com.yaca.android.ui.common.connectionColor
import com.yaca.android.ui.common.connectionLabel
import com.yaca.core.client.ConnectionState

/**
 * One computer in the list. The whole upper part opens it; the overflow holds the forget action.
 * A refused computer says why and offers to pair again, since retrying alone will not help.
 */
@Composable
fun HostCard(
    row: HostRow,
    onOpen: () -> Unit,
    onForget: () -> Unit,
    onPairAgain: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val connection = row.connection
    Card(
        modifier = modifier.fillMaxWidth(),
        shape = MaterialTheme.shapes.medium,
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surface),
        border = BorderStroke(1.dp, MaterialTheme.colorScheme.outlineVariant),
    ) {
        Column {
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .clickable(onClick = onOpen)
                    .padding(start = 16.dp, top = 12.dp, bottom = 12.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Icon(
                    imageVector = Icons.Filled.Computer,
                    contentDescription = null,
                    tint = MaterialTheme.colorScheme.primary,
                )
                Column(Modifier.weight(1f).padding(horizontal = 12.dp)) {
                    Text(row.record.name, style = MaterialTheme.typography.titleMedium)
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        ConnectionDot(connectionColor(connection))
                        Spacer(Modifier.width(6.dp))
                        Text(
                            text = connectionLabel(connection),
                            style = MaterialTheme.typography.bodySmall,
                            color = connectionColor(connection),
                        )
                    }
                }
                ForgetMenu(onForget = onForget)
            }
            if (connection is ConnectionState.Refused) {
                Column(Modifier.padding(start = 16.dp, end = 16.dp, bottom = 8.dp)) {
                    Text(
                        text = refusalMessage(connection.reason),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.error,
                    )
                    TextButton(onClick = onPairAgain) {
                        Text(stringResource(R.string.hosts_pair_again))
                    }
                }
            }
        }
    }
}
