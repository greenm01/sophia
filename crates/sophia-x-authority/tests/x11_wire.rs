use sophia_protocol::{
    BufferSource, ClientAdmissionContext, ClientAdmissionId, ClientAuthProvenance,
    ClientAuthenticationMethod, DeviceId, InputEventKind, NamespaceCapabilities, NamespaceContext,
    NamespaceId, NamespacePortalCapability, NamespaceProfile, OutputId, OutputTopologyEntry,
    OutputTopologySnapshot, Point, PortalBrokerRequestPacket, PortalDecision, PortalGrant,
    PortalGrantState, PortalRequest, PortalTransfer, PortalTransferId, PortalTransferKind, Rect,
    Region, RoutedInputRequest, SeatId, Size, SurfaceConstraints, SurfaceId, TransactionId,
};
use sophia_x_authority::*;

#[cfg(unix)]
#[test]
fn x11_request_reader_receives_bounded_scm_rights_with_the_request_header() {
    use std::fs::File;
    use std::io::IoSlice;
    use std::mem::MaybeUninit;
    use std::os::fd::AsFd;
    use std::os::unix::net::UnixStream;

    let (sender, mut receiver) = UnixStream::pair().unwrap();
    let request =
        extension_query_version_request(XByteOrder::LittleEndian, X_DRI3_MAJOR_OPCODE, 1, 2);
    let file = File::open("/dev/null").unwrap();
    let borrowed = [file.as_fd()];
    let mut space = [MaybeUninit::uninit();
        rustix::cmsg_space!(ScmRights(sophia_protocol::DMA_BUF_MAX_PLANES))];
    let mut ancillary = rustix::net::SendAncillaryBuffer::new(&mut space);
    assert!(ancillary.push(rustix::net::SendAncillaryMessage::ScmRights(&borrowed)));
    let sent = rustix::net::sendmsg(
        sender,
        &[IoSlice::new(&request)],
        &mut ancillary,
        rustix::net::SendFlags::empty(),
    )
    .unwrap();
    assert_eq!(sent, request.len());

    let received = read_x11_core_request(&mut receiver, XByteOrder::LittleEndian)
        .unwrap()
        .unwrap();
    assert_eq!(received.major_opcode, X_DRI3_MAJOR_OPCODE);
    assert_eq!(received.bytes, request);
    assert_eq!(received.fds.len(), 1);
}

#[cfg(unix)]
#[test]
fn x11_output_record_sends_bounded_scm_rights_with_the_first_bytes() {
    use std::fs::File;
    use std::io::IoSliceMut;
    use std::mem::MaybeUninit;
    use std::os::fd::OwnedFd;
    use std::os::unix::net::UnixStream;

    for fd_count in [1, sophia_protocol::DMA_BUF_MAX_PLANES] {
        let (mut sender, receiver) = UnixStream::pair().unwrap();
        let payload = vec![0x5a; X_CLIENT_OUTPUT_RECORD_LEN];
        let fds = (0..fd_count)
            .map(|_| OwnedFd::from(File::open("/dev/null").unwrap()))
            .collect();
        let record = X11SocketOutputRecord::new(payload.clone(), fds).unwrap();
        assert_eq!(record.bytes(), payload);
        assert_eq!(record.fd_count(), fd_count);

        write_x11_socket_output_record(&mut sender, record).unwrap();

        let mut bytes = [0; X_CLIENT_OUTPUT_RECORD_LEN];
        let mut iov = [IoSliceMut::new(&mut bytes)];
        let mut ancillary_space = [MaybeUninit::uninit();
            rustix::cmsg_space!(ScmRights(sophia_protocol::DMA_BUF_MAX_PLANES))];
        let mut ancillary = rustix::net::RecvAncillaryBuffer::new(&mut ancillary_space);
        let received = rustix::net::recvmsg(
            receiver,
            &mut iov,
            &mut ancillary,
            rustix::net::RecvFlags::CMSG_CLOEXEC,
        )
        .unwrap();
        assert_eq!(received.bytes, payload.len());
        assert_eq!(bytes, payload.as_slice());
        let received_fds = ancillary
            .drain()
            .flat_map(|message| match message {
                rustix::net::RecvAncillaryMessage::ScmRights(fds) => fds.collect::<Vec<_>>(),
                _ => Vec::new(),
            })
            .collect::<Vec<_>>();
        assert_eq!(received_fds.len(), fd_count);
        for fd in received_fds {
            File::from(fd).metadata().unwrap();
        }
    }
}

#[cfg(unix)]
#[test]
fn x11_output_record_rejects_empty_bytes_and_excess_descriptors() {
    use std::fs::File;
    use std::os::fd::OwnedFd;

    assert!(X11SocketOutputRecord::new(Vec::new(), Vec::new()).is_err());
    let fds = (0..=sophia_protocol::DMA_BUF_MAX_PLANES)
        .map(|_| OwnedFd::from(File::open("/dev/null").unwrap()))
        .collect();
    let error = X11SocketOutputRecord::new(vec![0], fds).unwrap_err();
    assert!(error.to_string().contains("maximum is"));
}

#[cfg(unix)]
#[test]
fn x11_output_record_preserves_byte_only_output() {
    use std::io::Read;
    use std::os::unix::net::UnixStream;

    let (mut sender, mut receiver) = UnixStream::pair().unwrap();
    let payload = vec![0xa5; X_CLIENT_OUTPUT_RECORD_LEN];
    let record = X11SocketOutputRecord::try_from(payload.clone()).unwrap();
    write_x11_socket_output_record(&mut sender, record).unwrap();
    let mut observed = vec![0; payload.len()];
    receiver.read_exact(&mut observed).unwrap();
    assert_eq!(observed, payload);
}

#[test]
fn mit_shm_completion_uses_the_advertised_extension_event_layout() {
    for byte_order in [XByteOrder::LittleEndian, XByteOrder::BigEndian] {
        let event = encode_x_client_event(
            byte_order,
            XClientEvent::ShmCompletion {
                sequence: 0x1234,
                drawable: XResourceId::new(0x220701, 1),
                segment: XResourceId::new(0x440001, 1),
                offset: 128,
            },
        );
        assert_eq!(event[0], X_MIT_SHM_FIRST_EVENT);
        assert_eq!(read_u16(byte_order, &event[2..4]), 0x1234);
        assert_eq!(read_u32(byte_order, &event[4..8]), 0x220701);
        assert_eq!(
            read_u16(byte_order, &event[8..10]),
            u16::from(X_MIT_SHM_PUT_IMAGE_MINOR_OPCODE)
        );
        assert_eq!(event[10], X_MIT_SHM_MAJOR_OPCODE);
        assert_eq!(read_u32(byte_order, &event[12..16]), 0x440001);
        assert_eq!(read_u32(byte_order, &event[16..20]), 128);
    }
}

#[test]
fn present_complete_and_idle_notifications_use_xge_packed_layouts() {
    for byte_order in [XByteOrder::LittleEndian, XByteOrder::BigEndian] {
        let complete = encode_x_client_event(
            byte_order,
            XClientEvent::PresentCompleteNotify {
                sequence: 0x1234,
                event_id: XResourceId::new(0x220900, 1),
                window: XResourceId::new(0x220901, 1),
                serial: 77,
                ust: 123_456,
                msc: 42,
                mode: 1,
            },
        );
        assert_eq!(complete.len(), 44);
        assert_eq!(complete[0], 35);
        assert_eq!(complete[1], X_PRESENT_MAJOR_OPCODE);
        assert_eq!(read_u32(byte_order, &complete[4..8]), 3);
        assert_eq!(read_u16(byte_order, &complete[8..10]), 1);
        assert_eq!(complete[10], 0);
        assert_eq!(complete[11], 1);
        assert_eq!(read_u32(byte_order, &complete[12..16]), 0x220900);
        assert_eq!(read_u32(byte_order, &complete[16..20]), 0x220901);
        assert_eq!(read_u32(byte_order, &complete[20..24]), 77);
        assert_eq!(read_u64(byte_order, &complete[24..32]), 123_456);
        assert_eq!(read_u64(byte_order, &complete[36..44]), 42);

        let idle = encode_x_client_event(
            byte_order,
            XClientEvent::PresentIdleNotify {
                sequence: 0x1235,
                event_id: XResourceId::new(0x220900, 1),
                window: XResourceId::new(0x220901, 1),
                serial: 77,
                pixmap: XResourceId::new(0x220902, 1),
                idle_fence: Some(XResourceId::new(0x220903, 1)),
            },
        );
        assert_eq!(idle.len(), 36);
        assert_eq!(idle[0], 35);
        assert_eq!(idle[1], X_PRESENT_MAJOR_OPCODE);
        assert_eq!(read_u32(byte_order, &idle[4..8]), 1);
        assert_eq!(read_u16(byte_order, &idle[8..10]), 2);
        assert_eq!(read_u32(byte_order, &idle[24..28]), 0x220902);
        assert_eq!(read_u32(byte_order, &idle[28..32]), 0x220903);
    }
}

#[test]
fn selection_request_and_clear_events_use_core_x11_layout() {
    let owner = XResourceId::new(0x200001, 1);
    let requestor = XResourceId::new(0x400001, 1);
    for byte_order in [XByteOrder::LittleEndian, XByteOrder::BigEndian] {
        let clear = encode_x_client_event(
            byte_order,
            XClientEvent::SelectionClear {
                sequence: 9,
                time: 11,
                owner,
                selection: 12,
            },
        );
        assert_eq!(clear[0], 29);
        assert_eq!(read_u32(byte_order, &clear[4..8]), 11);
        assert_eq!(read_u32(byte_order, &clear[8..12]), 0x200001);
        assert_eq!(read_u32(byte_order, &clear[12..16]), 12);

        let request = encode_x_client_event(
            byte_order,
            XClientEvent::SelectionRequest {
                sequence: 10,
                time: 13,
                owner,
                requestor,
                selection: 14,
                target: 15,
                property: 16,
            },
        );
        assert_eq!(request[0], 30);
        assert_eq!(read_u32(byte_order, &request[8..12]), 0x200001);
        assert_eq!(read_u32(byte_order, &request[12..16]), 0x400001);
        assert_eq!(read_u32(byte_order, &request[16..20]), 14);
        assert_eq!(read_u32(byte_order, &request[20..24]), 15);
        assert_eq!(read_u32(byte_order, &request[24..28]), 16);
    }
}

#[test]
fn send_event_accepts_selection_notify_and_rejects_input_events() {
    let namespace = NamespaceId::from_raw(44);
    let byte_order = XByteOrder::LittleEndian;
    let mut request = vec![0; 44];
    request[0] = 25;
    request[2..4].copy_from_slice(&11u16.to_le_bytes());
    request[4..8].copy_from_slice(&0x200001u32.to_le_bytes());
    request[12] = 31;
    request[16..20].copy_from_slice(&17u32.to_le_bytes());
    request[20..24].copy_from_slice(&0x200001u32.to_le_bytes());
    request[24..28].copy_from_slice(&18u32.to_le_bytes());
    request[28..32].copy_from_slice(&19u32.to_le_bytes());
    request[32..36].copy_from_slice(&20u32.to_le_bytes());

    assert_eq!(
        decode_x11_core_request(context(namespace, 1, byte_order), &request).unwrap(),
        XWireRequest::SendSelectionNotify {
            destination: XResourceId::new(0x200001, 1),
            event_mask: 0,
            event: XClientEvent::SelectionNotify {
                sequence: 0,
                time: 17,
                requestor: XResourceId::new(0x200001, 1),
                selection: 18,
                target: 19,
                property: 20,
            },
        }
    );

    request[12] = 2;
    assert_eq!(
        decode_x11_core_request(context(namespace, 1, byte_order), &request),
        Err(XWireParseError::InvalidEventType(2))
    );
}

#[test]
fn x11_setup_parser_accepts_little_endian_auth_fields() {
    let bytes = setup_request(
        XByteOrder::LittleEndian,
        11,
        0,
        b"MIT-MAGIC-COOKIE-1",
        b"0123456789abcdef",
    );

    let request = parse_x11_setup_request(&bytes).unwrap();

    assert_eq!(request.byte_order, XByteOrder::LittleEndian);
    assert_eq!(request.major_version, 11);
    assert_eq!(request.minor_version, 0);
    assert_eq!(request.authorization_protocol_name, b"MIT-MAGIC-COOKIE-1");
    assert_eq!(request.authorization_data, b"0123456789abcdef");
    assert_eq!(
        x11_setup_request_total_len(&bytes[..12]).unwrap(),
        bytes.len()
    );
}

#[test]
fn x11_setup_parser_accepts_big_endian_empty_auth() {
    let bytes = setup_request(XByteOrder::BigEndian, 11, 0, b"", b"");

    let request = parse_x11_setup_request(&bytes).unwrap();

    assert_eq!(request.byte_order, XByteOrder::BigEndian);
    assert_eq!(request.major_version, 11);
    assert!(request.authorization_protocol_name.is_empty());
    assert!(request.authorization_data.is_empty());
}

#[test]
fn x11_setup_parser_rejects_malformed_inputs() {
    assert_eq!(
        parse_x11_setup_request(&[b'l'; 4]),
        Err(XSetupParseError::Truncated {
            needed: 12,
            actual: 4
        })
    );
    assert_eq!(
        parse_x11_setup_request(&setup_request(XByteOrder::LittleEndian, 12, 0, b"", b"")),
        Err(XSetupParseError::UnsupportedMajorVersion(12))
    );
    assert_eq!(
        parse_x11_setup_request(&[b'x', 0, 11, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
        Err(XSetupParseError::InvalidByteOrder(b'x'))
    );

    let mut overlarge = setup_request(XByteOrder::LittleEndian, 11, 0, b"", b"");
    overlarge[6..8].copy_from_slice(&1025u16.to_le_bytes());
    assert_eq!(
        parse_x11_setup_request(&overlarge),
        Err(XSetupParseError::AuthFieldTooLarge {
            field: "authorization_protocol_name",
            len: 1025,
            max: X_SETUP_MAX_AUTH_FIELD_LEN,
        })
    );

    let mut truncated = setup_request(XByteOrder::LittleEndian, 11, 0, b"AUTH", b"DATA");
    truncated.pop();
    assert!(matches!(
        parse_x11_setup_request(&truncated),
        Err(XSetupParseError::Truncated { .. })
    ));
}

#[test]
fn x11_setup_success_reply_encodes_resource_id_facts() {
    let reply = encode_x11_setup_success(
        XByteOrder::LittleEndian,
        &XSetupSuccess {
            major_version: 11,
            minor_version: 0,
            release: 7,
            resource_id_base: 0x0020_0000,
            resource_id_mask: 0x001f_ffff,
            max_request_units: 4096,
            vendor: b"Sophia".to_vec(),
            roots: 0,
            formats: 0,
            root_size: Size {
                width: 1280,
                height: 720,
            },
        },
    )
    .unwrap();

    assert_eq!(reply[0], 1);
    assert_eq!(read_u16(XByteOrder::LittleEndian, &reply[2..4]), 11);
    assert_eq!(read_u16(XByteOrder::LittleEndian, &reply[4..6]), 0);
    assert_eq!(read_u32(XByteOrder::LittleEndian, &reply[8..12]), 7);
    assert_eq!(
        read_u32(XByteOrder::LittleEndian, &reply[12..16]),
        0x0020_0000
    );
    assert_eq!(
        read_u32(XByteOrder::LittleEndian, &reply[16..20]),
        0x001f_ffff
    );
    assert_eq!(read_u16(XByteOrder::LittleEndian, &reply[24..26]), 6);
    assert_eq!(&reply[40..46], b"Sophia");
}

#[test]
fn x11_setup_success_reply_can_advertise_minimal_root_screen() {
    let reply = encode_x11_setup_success(
        XByteOrder::LittleEndian,
        &XSetupSuccess::client_compatible(),
    )
    .unwrap();

    assert_eq!(reply[0], 1);
    assert_eq!(reply[28], 1);
    assert_eq!(reply[29], 1);
    assert_eq!(read_u32(XByteOrder::LittleEndian, &reply[56..60]), 0x20);
    assert_eq!(read_u32(XByteOrder::LittleEndian, &reply[88..92]), 0x22);
    assert_eq!(reply[94], 24);
    assert_eq!(reply[95], 1);
}

#[test]
fn x11_setup_failure_reply_encodes_native_failure() {
    let reply = encode_x11_setup_failure(
        XByteOrder::BigEndian,
        &XSetupFailure {
            major_version: 11,
            minor_version: 0,
            reason: b"unsupported".to_vec(),
        },
    )
    .unwrap();

    assert_eq!(reply[0], 0);
    assert_eq!(reply[1], b"unsupported".len() as u8);
    assert_eq!(read_u16(XByteOrder::BigEndian, &reply[2..4]), 11);
    assert_eq!(&reply[8..19], b"unsupported");
    assert_eq!(reply.len() % 4, 0);
}

#[test]
fn x11_core_decoder_maps_create_and_map_to_authority_packets() {
    let namespace = NamespaceId::from_raw(41);
    let create = decode_x11_core_request(
        context(namespace, 501, XByteOrder::LittleEndian),
        &create_window_request(XByteOrder::LittleEndian, 0x220001, 10, 20, 640, 480),
    )
    .unwrap();
    let map = decode_x11_core_request(
        context(namespace, 502, XByteOrder::LittleEndian),
        &resource_request(XByteOrder::LittleEndian, 8, 0x220001),
    )
    .unwrap();
    let map_subwindows = decode_x11_core_request(
        context(namespace, 503, XByteOrder::LittleEndian),
        &resource_request(XByteOrder::LittleEndian, 9, X_SETUP_DEFAULT_ROOT),
    )
    .unwrap();
    let unmap = decode_x11_core_request(
        context(namespace, 504, XByteOrder::LittleEndian),
        &resource_request(XByteOrder::LittleEndian, 10, 0x220001),
    )
    .unwrap();
    let configure = decode_x11_core_request(
        context(namespace, 505, XByteOrder::LittleEndian),
        &configure_window_request(XByteOrder::LittleEndian, 0x220001, 0x000c, &[12, 14]),
    )
    .unwrap();
    let attributes = decode_x11_core_request(
        context(namespace, 506, XByteOrder::LittleEndian),
        &change_window_attributes_request(XByteOrder::LittleEndian, X_SETUP_DEFAULT_ROOT),
    )
    .unwrap();
    let get_attributes = decode_x11_core_request(
        context(namespace, 507, XByteOrder::LittleEndian),
        &resource_request(XByteOrder::LittleEndian, 3, X_SETUP_DEFAULT_ROOT),
    )
    .unwrap();

    let XWireRequest::CreateWindow {
        packet: create,
        background_pixel,
        event_mask,
        do_not_propagate_mask,
        parent,
    } = create
    else {
        panic!("expected create-window request");
    };
    assert_eq!(background_pixel, None);
    assert_eq!(event_mask, None);
    assert_eq!(do_not_propagate_mask, None);
    assert_eq!(parent, XResourceId::new(u64::from(X_SETUP_DEFAULT_ROOT), 1));
    assert_eq!(create.namespace, namespace);
    assert_eq!(
        create.kind,
        XAuthorityRequestKind::CreateWindow {
            window: XResourceId::new(0x220001, 1),
            surface: SurfaceId::new(0x220001, 1),
            geometry: Rect {
                x: 10,
                y: 20,
                width: 640,
                height: 480,
            },
            constraints: SurfaceConstraints {
                min_size: None,
                max_size: None,
            },
            generation: 1,
        }
    );

    let XWireRequest::Authority(map) = map else {
        panic!("expected authority request");
    };
    assert_eq!(
        map.kind,
        XAuthorityRequestKind::MapWindow {
            window: XResourceId::new(0x220001, 1),
            generation: 2,
        }
    );
    assert_eq!(
        map_subwindows,
        XWireRequest::MapSubwindows {
            window: XResourceId::new(u64::from(X_SETUP_DEFAULT_ROOT), 1),
        }
    );
    assert_eq!(
        unmap,
        XWireRequest::UnmapWindow {
            window: XResourceId::new(0x220001, 1),
        }
    );
    assert_eq!(
        configure,
        XWireRequest::ConfigureWindow {
            window: XResourceId::new(0x220001, 1),
            value_mask: 0x000c,
            x: None,
            y: None,
            width: Some(12),
            height: Some(14),
            sibling: None,
            stack_mode: None,
        }
    );
    let geometry = decode_x11_core_request(
        context(namespace, 508, XByteOrder::LittleEndian),
        &resource_request(XByteOrder::LittleEndian, 14, X_SETUP_DEFAULT_ROOT),
    )
    .unwrap();
    let tree = decode_x11_core_request(
        context(namespace, 509, XByteOrder::LittleEndian),
        &resource_request(XByteOrder::LittleEndian, 15, X_SETUP_DEFAULT_ROOT),
    )
    .unwrap();
    let list_properties = decode_x11_core_request(
        context(namespace, 510, XByteOrder::LittleEndian),
        &resource_request(XByteOrder::LittleEndian, 21, X_SETUP_DEFAULT_ROOT),
    )
    .unwrap();
    assert_eq!(
        geometry,
        XWireRequest::GetGeometry {
            drawable: XResourceId::new(u64::from(X_SETUP_DEFAULT_ROOT), 1),
        }
    );
    assert_eq!(
        tree,
        XWireRequest::QueryTree {
            window: XResourceId::new(u64::from(X_SETUP_DEFAULT_ROOT), 1),
        }
    );
    assert_eq!(
        list_properties,
        XWireRequest::ListProperties {
            window: XResourceId::new(u64::from(X_SETUP_DEFAULT_ROOT), 1),
        }
    );
    let translate = decode_x11_core_request(
        context(namespace, 511, XByteOrder::LittleEndian),
        &translate_coordinates_request(
            XByteOrder::LittleEndian,
            X_SETUP_DEFAULT_ROOT,
            X_SETUP_DEFAULT_ROOT,
            12,
            34,
        ),
    )
    .unwrap();
    assert_eq!(
        translate,
        XWireRequest::TranslateCoordinates {
            source: XResourceId::new(u64::from(X_SETUP_DEFAULT_ROOT), 1),
            destination: XResourceId::new(u64::from(X_SETUP_DEFAULT_ROOT), 1),
            src_x: 12,
            src_y: 34,
        }
    );
    assert_eq!(
        get_attributes,
        XWireRequest::GetWindowAttributes {
            window: XResourceId::new(u64::from(X_SETUP_DEFAULT_ROOT), 1),
        }
    );
    assert_eq!(
        attributes,
        XWireRequest::ChangeWindowAttributes {
            window: XResourceId::new(u64::from(X_SETUP_DEFAULT_ROOT), 1),
            event_mask: None,
            do_not_propagate_mask: None,
        }
    );
    let modifier_mapping = decode_x11_core_request(
        context(namespace, 512, XByteOrder::LittleEndian),
        &[119, 0, 1, 0],
    )
    .unwrap();
    assert_eq!(modifier_mapping, XWireRequest::GetModifierMapping);
    let keyboard_mapping = decode_x11_core_request(
        context(namespace, 513, XByteOrder::LittleEndian),
        &[101, 0, 2, 0, 8, 4, 0, 0],
    )
    .unwrap();
    assert_eq!(
        keyboard_mapping,
        XWireRequest::GetKeyboardMapping {
            first_keycode: 8,
            count: 4,
        }
    );
}

#[test]
fn keyboard_mapping_request_uses_body_keycode_and_count_bytes() {
    let namespace = NamespaceId::from_raw(44);
    for (byte_order, request) in [
        (XByteOrder::LittleEndian, [101, 0, 2, 0, 8, 248, 0, 0]),
        (XByteOrder::BigEndian, [101, 0, 0, 2, 8, 248, 0, 0]),
    ] {
        assert_eq!(
            decode_x11_core_request(context(namespace, 514, byte_order), &request).unwrap(),
            XWireRequest::GetKeyboardMapping {
                first_keycode: 8,
                count: 248,
            }
        );
    }
}

#[test]
fn x11_core_decoder_preserves_window_background_pixel() {
    let namespace = NamespaceId::from_raw(44);
    for byte_order in [XByteOrder::LittleEndian, XByteOrder::BigEndian] {
        let create = decode_x11_core_request(
            context(namespace, 500, byte_order),
            &create_window_background_request(byte_order, 0x220002, 10, 20, 320, 200, 0x0012_3456),
        )
        .unwrap();
        let XWireRequest::CreateWindow {
            background_pixel, ..
        } = create
        else {
            panic!("expected create-window request");
        };
        assert_eq!(background_pixel, Some(0x0012_3456));
    }
}

#[test]
fn x11_core_decoder_captures_destroy_window_requests() {
    let namespace = NamespaceId::from_raw(41);
    let destroy = decode_x11_core_request(
        context(namespace, 502, XByteOrder::LittleEndian),
        &resource_request(XByteOrder::LittleEndian, 4, 0x220001),
    )
    .unwrap();

    assert_eq!(
        destroy,
        XWireRequest::DestroyWindow {
            window: XResourceId::new(0x220001, 1),
        }
    );
}

#[test]
fn x11_core_decoder_maps_selection_requests_to_authority_packets() {
    let namespace = NamespaceId::from_raw(42);
    let set_owner = decode_x11_core_request(
        context(namespace, 503, XByteOrder::BigEndian),
        &set_selection_owner_request(XByteOrder::BigEndian, 0x220001, 1, 10),
    )
    .unwrap();
    let convert = decode_x11_core_request(
        context(namespace, 504, XByteOrder::BigEndian),
        &convert_selection_request(XByteOrder::BigEndian, 0x220002, 1, 2, 3, 11),
    )
    .unwrap();
    let get_owner = decode_x11_core_request(
        context(namespace, 505, XByteOrder::BigEndian),
        &resource_request(XByteOrder::BigEndian, 23, 1),
    )
    .unwrap();
    let grab_button = decode_x11_core_request(
        context(namespace, 506, XByteOrder::BigEndian),
        &grab_button_request(
            XByteOrder::BigEndian,
            X_SETUP_DEFAULT_ROOT,
            0x001c,
            1,
            0x0040,
        ),
    )
    .unwrap();
    let ungrab_button = decode_x11_core_request(
        context(namespace, 507, XByteOrder::BigEndian),
        &ungrab_button_request(XByteOrder::BigEndian, X_SETUP_DEFAULT_ROOT, 1, 0x0040),
    )
    .unwrap();
    let grab = decode_x11_core_request(
        context(namespace, 508, XByteOrder::BigEndian),
        &[36, 0, 0, 1],
    )
    .unwrap();
    let ungrab = decode_x11_core_request(
        context(namespace, 509, XByteOrder::BigEndian),
        &[37, 0, 0, 1],
    )
    .unwrap();

    let XWireRequest::Authority(set_owner) = set_owner else {
        panic!("expected authority request");
    };
    assert_eq!(
        set_owner.kind,
        XAuthorityRequestKind::SetSelectionOwner {
            selection: 1,
            owner: Some(XResourceId::new(0x220001, 1)),
            timestamp: 10,
            selection_timestamp: 10,
            kind: XSelectionChangeKind::SetOwner,
        }
    );

    let XWireRequest::Authority(convert) = convert else {
        panic!("expected authority request");
    };
    assert_eq!(
        convert.kind,
        XAuthorityRequestKind::RequestSelection {
            requestor: XResourceId::new(0x220002, 1),
            selection: 1,
            target: 2,
            target_name: "atom:2".to_owned(),
            property: 3,
            time: 11,
            transfer: sophia_protocol::PortalTransferId::from_raw(504),
        }
    );
    assert_eq!(get_owner, XWireRequest::GetSelectionOwner { selection: 1 });
    assert_eq!(
        grab_button,
        XWireRequest::GrabButton {
            window: XResourceId::new(u64::from(X_SETUP_DEFAULT_ROOT), 1),
            event_mask: 0x001c,
            button: 1,
            modifiers: 0x0040,
            owner_events: true,
            pointer_mode: 1,
            keyboard_mode: 1,
        }
    );
    assert_eq!(
        ungrab_button,
        XWireRequest::UngrabButton {
            window: XResourceId::new(u64::from(X_SETUP_DEFAULT_ROOT), 1),
            button: 1,
            modifiers: 0x0040,
        }
    );
    assert_eq!(grab, XWireRequest::GrabServer);
    assert_eq!(ungrab, XWireRequest::UngrabServer);
}

#[test]
fn active_keyboard_pointer_key_and_allow_events_requests_decode() {
    let namespace = NamespaceId::from_raw(45);
    let window = X_SETUP_DEFAULT_ROOT;
    let mut grab_pointer = vec![26, 1, 6, 0];
    grab_pointer.extend_from_slice(&window.to_le_bytes());
    grab_pointer.extend_from_slice(&0x004cu16.to_le_bytes());
    grab_pointer.extend_from_slice(&[1, 0]);
    grab_pointer.extend_from_slice(&[0; 8]);
    grab_pointer.extend_from_slice(&7u32.to_le_bytes());
    assert_eq!(
        decode_x11_core_request(
            context(namespace, 1, XByteOrder::LittleEndian),
            &grab_pointer
        )
        .unwrap(),
        XWireRequest::GrabPointer {
            window: XResourceId::new(u64::from(window), 1),
            event_mask: 0x004c,
            owner_events: true,
            pointer_mode: 1,
            keyboard_mode: 0,
            time: 7,
        }
    );
    let mut grab_keyboard = vec![31, 0, 4, 0];
    grab_keyboard.extend_from_slice(&window.to_le_bytes());
    grab_keyboard.extend_from_slice(&8u32.to_le_bytes());
    grab_keyboard.extend_from_slice(&[0, 1, 0, 0]);
    assert_eq!(
        decode_x11_core_request(
            context(namespace, 2, XByteOrder::LittleEndian),
            &grab_keyboard
        )
        .unwrap(),
        XWireRequest::GrabKeyboard {
            window: XResourceId::new(u64::from(window), 1),
            owner_events: false,
            pointer_mode: 0,
            keyboard_mode: 1,
            time: 8,
        }
    );
    let mut grab_key = vec![33, 1, 4, 0];
    grab_key.extend_from_slice(&window.to_le_bytes());
    grab_key.extend_from_slice(&0x8000u16.to_le_bytes());
    grab_key.extend_from_slice(&[38, 1, 0, 0, 0, 0]);
    assert_eq!(
        decode_x11_core_request(context(namespace, 3, XByteOrder::LittleEndian), &grab_key)
            .unwrap(),
        XWireRequest::GrabKey {
            window: XResourceId::new(u64::from(window), 1),
            key: 38,
            modifiers: 0x8000,
            owner_events: true,
            pointer_mode: 1,
            keyboard_mode: 0,
        }
    );
    let allow = [35, 6, 2, 0, 9, 0, 0, 0];
    assert_eq!(
        decode_x11_core_request(context(namespace, 4, XByteOrder::LittleEndian), &allow).unwrap(),
        XWireRequest::AllowEvents { mode: 6, time: 9 }
    );
}

#[test]
fn x11_core_decoder_captures_change_property_and_table_updates() {
    let namespace = NamespaceId::from_raw(43);
    let decoded = decode_x11_core_request(
        context(namespace, 505, XByteOrder::LittleEndian),
        &change_property_request(
            XByteOrder::LittleEndian,
            XPropertyMode::Replace,
            0x220003,
            7,
            8,
            8,
            b"hello",
        ),
    )
    .unwrap();
    let XWireRequest::ChangeProperty(change) = decoded else {
        panic!("expected property change");
    };

    let mut properties = XPropertyTable::new();
    let record = properties.apply_change(namespace, change).unwrap();

    assert_eq!(record.window, XResourceId::new(0x220003, 1));
    assert_eq!(record.property, 7);
    assert_eq!(record.property_type, 8);
    assert_eq!(record.format, 8);
    assert_eq!(record.bytes, b"hello");
    assert_eq!(record.generation, 1);
}

#[test]
fn x11_property_table_appends_and_rejects_type_mismatch() {
    let namespace = NamespaceId::from_raw(44);
    let mut properties = XPropertyTable::new();
    let window = XResourceId::new(0x220004, 1);

    properties
        .apply_change(
            namespace,
            XPropertyChange {
                mode: XPropertyMode::Replace,
                window,
                property: 7,
                property_type: 8,
                format: 8,
                bytes: b"hello".to_vec(),
            },
        )
        .unwrap();
    let appended = properties
        .apply_change(
            namespace,
            XPropertyChange {
                mode: XPropertyMode::Append,
                window,
                property: 7,
                property_type: 8,
                format: 8,
                bytes: b" world".to_vec(),
            },
        )
        .unwrap();

    assert_eq!(appended.bytes, b"hello world");
    assert_eq!(appended.generation, 2);
    assert_eq!(
        properties.apply_change(
            namespace,
            XPropertyChange {
                mode: XPropertyMode::Append,
                window,
                property: 7,
                property_type: 9,
                format: 8,
                bytes: b"!".to_vec(),
            },
        ),
        Err(XPropertyError::TypeMismatch)
    );
}

#[test]
fn x11_atom_table_resolves_predefined_and_dynamic_names() {
    let mut atoms = XAtomTable::new();

    assert_eq!(atoms.atom(X_ATOM_NAME_WM_CLASS), Some(X_ATOM_WM_CLASS));
    assert_eq!(atoms.name(X_ATOM_WM_NAME), Some(X_ATOM_NAME_WM_NAME));
    assert_eq!(
        atoms.atom(X_ATOM_NAME_RESOURCE_MANAGER),
        Some(X_ATOM_RESOURCE_MANAGER)
    );

    let net_wm_name = atoms.intern(X_ATOM_NAME_NET_WM_NAME, false).unwrap();
    assert!(net_wm_name.is_some());
    assert_eq!(
        atoms.intern(X_ATOM_NAME_NET_WM_NAME, true).unwrap(),
        net_wm_name
    );
    assert!(atoms.intern("SOPHIA PRINTABLE", false).unwrap().is_some());
    assert_eq!(atoms.intern("SOPHIA_UNKNOWN", true).unwrap(), None);
    assert!(atoms.intern("", false).is_err());
}

#[test]
fn x11_core_decoder_captures_atom_requests() {
    let namespace = NamespaceId::from_raw(45);
    let intern = decode_x11_core_request(
        context(namespace, 506, XByteOrder::LittleEndian),
        &intern_atom_request(XByteOrder::LittleEndian, false, X_ATOM_NAME_NET_WM_NAME),
    )
    .unwrap();
    assert_eq!(
        intern,
        XWireRequest::InternAtom {
            only_if_exists: false,
            name: X_ATOM_NAME_NET_WM_NAME.to_owned(),
        }
    );

    let get_name = decode_x11_core_request(
        context(namespace, 507, XByteOrder::BigEndian),
        &get_atom_name_request(XByteOrder::BigEndian, X_ATOM_WM_CLASS),
    )
    .unwrap();
    assert_eq!(
        get_name,
        XWireRequest::GetAtomName {
            atom: X_ATOM_WM_CLASS
        }
    );
}

#[test]
fn x11_core_decoder_captures_get_property_requests() {
    let namespace = NamespaceId::from_raw(45);
    let get_property = decode_x11_core_request(
        context(namespace, 507, XByteOrder::LittleEndian),
        &get_property_request(
            XByteOrder::LittleEndian,
            false,
            0x220007,
            X_ATOM_WM_NAME,
            X_PROPERTY_ANY_TYPE,
            1,
            2,
        ),
    )
    .unwrap();

    assert_eq!(
        get_property,
        XWireRequest::GetProperty(XPropertyRead {
            delete: false,
            window: XResourceId::new(0x220007, 1),
            property: X_ATOM_WM_NAME,
            property_type: X_PROPERTY_ANY_TYPE,
            long_offset: 1,
            long_length: 2,
        })
    );
}

#[test]
fn x11_core_decoder_captures_create_gc_requests() {
    let namespace = NamespaceId::from_raw(45);
    let create_gc = decode_x11_core_request(
        context(namespace, 507, XByteOrder::LittleEndian),
        &create_gc_request(XByteOrder::LittleEndian, 0x220010, X_SETUP_DEFAULT_ROOT),
    )
    .unwrap();

    assert_eq!(
        create_gc,
        XWireRequest::CreateGraphicsContext {
            gc: XResourceId::new(0x220010, 1),
            drawable: XResourceId::new(u64::from(X_SETUP_DEFAULT_ROOT), 1),
            values: XGraphicsContextValues::default(),
        }
    );

    let clip = decode_x11_core_request(
        context(namespace, 508, XByteOrder::LittleEndian),
        &set_clip_rectangles_request(XByteOrder::LittleEndian, 0x220010, &[(2, 3, 20, 10)]),
    )
    .unwrap();
    assert_eq!(
        clip,
        XWireRequest::SetClipRectangles {
            gc: XResourceId::new(0x220010, 1),
            rectangles: vec![Rect {
                x: 2,
                y: 3,
                width: 20,
                height: 10,
            }],
        }
    );

    let clear = decode_x11_core_request(
        context(namespace, 509, XByteOrder::LittleEndian),
        &clear_area_request(XByteOrder::LittleEndian, true, 0x220010, 3, 4, 40, 30),
    )
    .unwrap();

    assert_eq!(
        clear,
        XWireRequest::ClearArea {
            exposures: true,
            window: XResourceId::new(0x220010, 1),
            x: 3,
            y: 4,
            width: 40,
            height: 30,
        }
    );
}

#[test]
fn x11_core_decoder_preserves_gc_raster_values_in_both_byte_orders() {
    let namespace = NamespaceId::from_raw(45);
    for byte_order in [XByteOrder::LittleEndian, XByteOrder::BigEndian] {
        let request = create_gc_values_request(
            byte_order,
            0x220020,
            X_SETUP_DEFAULT_ROOT,
            6,
            0x00ff_00ff,
            0x0012_3456,
            0x0065_4321,
            3,
            0x220021,
        );
        let decoded =
            decode_x11_core_request(context(namespace, 508, byte_order), &request).unwrap();
        let XWireRequest::CreateGraphicsContext { values, .. } = decoded else {
            panic!("expected CreateGC");
        };
        assert_eq!(values.function, 6);
        assert_eq!(values.plane_mask, 0x00ff_00ff);
        assert_eq!(values.foreground, 0x0012_3456);
        assert_eq!(values.background, 0x0065_4321);
        assert_eq!(values.line_width, 3);
        assert_eq!(values.font, Some(XResourceId::new(0x220021, 1)));
    }
}

#[test]
fn x11_core_decoder_captures_poly_fill_rectangle_requests() {
    let namespace = NamespaceId::from_raw(45);
    let fill = decode_x11_core_request(
        context(namespace, 507, XByteOrder::LittleEndian),
        &poly_fill_rectangle_request(
            XByteOrder::LittleEndian,
            0x220010,
            0x220011,
            &[(5, 6, 40, 30), (10, 12, 8, 9)],
        ),
    )
    .unwrap();

    assert_eq!(
        fill,
        XWireRequest::PolyFillRectangle {
            drawable: XResourceId::new(0x220010, 1),
            gc: XResourceId::new(0x220011, 1),
            rectangles: vec![
                Rect {
                    x: 5,
                    y: 6,
                    width: 40,
                    height: 30,
                },
                Rect {
                    x: 10,
                    y: 12,
                    width: 8,
                    height: 9,
                },
            ],
        }
    );

    let segments = decode_x11_core_request(
        context(namespace, 508, XByteOrder::LittleEndian),
        &poly_segment_request(
            XByteOrder::LittleEndian,
            0x220010,
            0x220011,
            &[(5, 6, 15, 16), (20, 30, 10, 24)],
        ),
    )
    .unwrap();

    assert_eq!(
        segments,
        XWireRequest::PolySegment {
            drawable: XResourceId::new(0x220010, 1),
            gc: XResourceId::new(0x220011, 1),
            damage: vec![
                Rect {
                    x: 5,
                    y: 6,
                    width: 11,
                    height: 11,
                },
                Rect {
                    x: 10,
                    y: 24,
                    width: 11,
                    height: 7,
                },
            ],
        }
    );

    let line = decode_x11_core_request(
        context(namespace, 509, XByteOrder::LittleEndian),
        &poly_line_request(
            XByteOrder::LittleEndian,
            0x220010,
            0x220011,
            &[(3, 4), (13, 9), (8, 20)],
        ),
    )
    .unwrap();

    assert_eq!(
        line,
        XWireRequest::PolyLine {
            drawable: XResourceId::new(0x220010, 1),
            gc: XResourceId::new(0x220011, 1),
            points: vec![
                XPoint { x: 3, y: 4 },
                XPoint { x: 13, y: 9 },
                XPoint { x: 8, y: 20 },
            ],
        }
    );

    let fill_poly = decode_x11_core_request(
        context(namespace, 510, XByteOrder::LittleEndian),
        &fill_poly_request(
            XByteOrder::LittleEndian,
            0x220010,
            0x220011,
            &[(5, 6), (15, 16), (8, 20)],
        ),
    )
    .unwrap();

    assert_eq!(
        fill_poly,
        XWireRequest::FillPoly {
            drawable: XResourceId::new(0x220010, 1),
            gc: XResourceId::new(0x220011, 1),
            damage: Some(Rect {
                x: 5,
                y: 6,
                width: 11,
                height: 15,
            }),
        }
    );

    let fill_arcs = decode_x11_core_request(
        context(namespace, 511, XByteOrder::LittleEndian),
        &poly_fill_arc_request(
            XByteOrder::LittleEndian,
            0x220010,
            0x220011,
            &[(7, 8, 41, 31, 0, 23040)],
        ),
    )
    .unwrap();

    assert_eq!(
        fill_arcs,
        XWireRequest::PolyFillArc {
            drawable: XResourceId::new(0x220010, 1),
            gc: XResourceId::new(0x220011, 1),
            damage: vec![Rect {
                x: 7,
                y: 8,
                width: 41,
                height: 31,
            }],
        }
    );

    let text = decode_x11_core_request(
        context(namespace, 512, XByteOrder::LittleEndian),
        &poly_text8_request(XByteOrder::LittleEndian, 0x220010, 0x220011, 5, 16, b"Hi"),
    )
    .unwrap();

    assert_eq!(
        text,
        XWireRequest::PolyText8 {
            drawable: XResourceId::new(0x220010, 1),
            gc: XResourceId::new(0x220011, 1),
            x: 5,
            y: 16,
            text: b"Hi".to_vec(),
        }
    );

    let padded_text = decode_x11_core_request(
        context(namespace, 513, XByteOrder::LittleEndian),
        &poly_text8_request(XByteOrder::LittleEndian, 0x220010, 0x220011, 5, 16, b"="),
    )
    .unwrap();

    assert_eq!(
        padded_text,
        XWireRequest::PolyText8 {
            drawable: XResourceId::new(0x220010, 1),
            gc: XResourceId::new(0x220011, 1),
            x: 5,
            y: 16,
            text: b"=".to_vec(),
        }
    );

    let compact_text = decode_x11_core_request(
        context(namespace, 514, XByteOrder::LittleEndian),
        &poly_text8_compact_item_request(
            XByteOrder::LittleEndian,
            0x220010,
            0x220011,
            5,
            16,
            b"Hi",
        ),
    )
    .unwrap();

    assert_eq!(
        compact_text,
        XWireRequest::PolyText8 {
            drawable: XResourceId::new(0x220010, 1),
            gc: XResourceId::new(0x220011, 1),
            x: 5,
            y: 16,
            text: b"Hi".to_vec(),
        }
    );

    let image_text = decode_x11_core_request(
        context(namespace, 515, XByteOrder::LittleEndian),
        &image_text8_request(XByteOrder::LittleEndian, 0x220010, 0x220011, 5, 16, b"Hi"),
    )
    .unwrap();

    assert_eq!(
        image_text,
        XWireRequest::ImageText8 {
            drawable: XResourceId::new(0x220010, 1),
            gc: XResourceId::new(0x220011, 1),
            x: 5,
            y: 16,
            text: b"Hi".to_vec(),
        }
    );
}

#[test]
fn x11_core_decoder_captures_put_image_requests() {
    let namespace = NamespaceId::from_raw(45);
    let put = decode_x11_core_request(
        context(namespace, 508, XByteOrder::LittleEndian),
        &put_image_request(
            XByteOrder::LittleEndian,
            0x220020,
            0x220021,
            8,
            4,
            3,
            5,
            &[0xaa; 128],
        ),
    )
    .unwrap();

    assert_eq!(
        put,
        XWireRequest::PutImage {
            format: 2,
            drawable: XResourceId::new(0x220020, 1),
            gc: XResourceId::new(0x220021, 1),
            width: 8,
            height: 4,
            dst_x: 3,
            dst_y: 5,
            left_pad: 0,
            depth: 24,
            data: vec![0xaa; 128],
        }
    );
}

#[test]
fn x11_core_decoder_rejects_out_of_range_client_resource_creators() {
    let namespace = NamespaceId::from_raw(45);
    let context = XWireClientContext {
        byte_order: XByteOrder::LittleEndian,
        namespace,
        transaction: TransactionId::from_raw(509),
        resource_id_range: Some(XWireClientResourceRange {
            base: X_SETUP_DEFAULT_RESOURCE_ID_BASE,
            mask: X_SETUP_DEFAULT_RESOURCE_ID_MASK,
        }),
    };
    let outside_range = 0x0040_0001;
    let requests = [
        create_window_request(XByteOrder::LittleEndian, outside_range, 1, 2, 300, 200),
        create_gc_request(
            XByteOrder::LittleEndian,
            outside_range,
            X_SETUP_DEFAULT_ROOT,
        ),
        create_pixmap_request(
            XByteOrder::LittleEndian,
            24,
            outside_range,
            X_SETUP_DEFAULT_ROOT,
            32,
            16,
        ),
        open_font_request(XByteOrder::LittleEndian, outside_range, "fixed"),
        create_colormap_request(
            XByteOrder::LittleEndian,
            outside_range,
            X_SETUP_DEFAULT_ROOT,
            X_SETUP_DEFAULT_VISUAL,
        ),
        create_glyph_cursor_request(
            XByteOrder::LittleEndian,
            outside_range,
            0x0020_0040,
            0x0020_0041,
        ),
        mit_shm_attach_request(XByteOrder::LittleEndian, outside_range, 77, false),
    ];

    for request in requests {
        assert_eq!(
            decode_x11_core_request(context, &request),
            Err(XWireParseError::ResourceIdOutsideClientRange {
                resource_id: outside_range,
            })
        );
    }
}

#[test]
fn x11_classic_shared_x_allows_peer_operations_on_existing_resources() {
    let namespace = NamespaceId::from_raw(45);
    let creator = XWireClientContext {
        byte_order: XByteOrder::LittleEndian,
        namespace,
        transaction: TransactionId::from_raw(510),
        resource_id_range: Some(XWireClientResourceRange {
            base: X_SETUP_DEFAULT_RESOURCE_ID_BASE,
            mask: X_SETUP_DEFAULT_RESOURCE_ID_MASK,
        }),
    };
    let peer = XWireClientContext {
        byte_order: XByteOrder::LittleEndian,
        namespace,
        transaction: TransactionId::from_raw(511),
        resource_id_range: Some(XWireClientResourceRange {
            base: 0x0040_0000,
            mask: X_SETUP_DEFAULT_RESOURCE_ID_MASK,
        }),
    };
    let window = 0x0020_0001;
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();

    let create = decode_x11_core_request(
        creator,
        &create_window_request(XByteOrder::LittleEndian, window, 10, 20, 640, 480),
    )
    .unwrap();
    dispatch_x11_wire_request(
        dispatch_context(namespace, 1, XByteOrder::LittleEndian, 1),
        create,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );

    // The peer cannot create in the creator's range, but classic shared-X
    // deliberately permits it to operate on an existing same-namespace XID.
    let map = decode_x11_core_request(peer, &resource_request(XByteOrder::LittleEndian, 8, window))
        .unwrap();
    let mapped = dispatch_x11_wire_request(
        dispatch_context(namespace, 2, XByteOrder::LittleEndian, 8),
        map,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );

    assert!(mapped.outputs.iter().any(|output| {
        matches!(
            output,
            XClientOutput::Event(XClientEvent::MapNotify { window: notified, .. })
                if *notified == XResourceId::new(u64::from(window), 1)
        )
    }));
}

#[test]
fn x11_core_decoder_captures_pixmap_and_copy_area_requests() {
    let namespace = NamespaceId::from_raw(45);
    let create = decode_x11_core_request(
        context(namespace, 571, XByteOrder::LittleEndian),
        &create_pixmap_request(XByteOrder::LittleEndian, 24, 0x220030, 0x220031, 32, 16),
    )
    .unwrap();
    assert_eq!(
        create,
        XWireRequest::CreatePixmap {
            depth: 24,
            pixmap: XResourceId::new(0x220030, 1),
            drawable: XResourceId::new(0x220031, 1),
            width: 32,
            height: 16,
        }
    );

    let copy = decode_x11_core_request(
        context(namespace, 572, XByteOrder::LittleEndian),
        &copy_area_request(
            XByteOrder::LittleEndian,
            0x220030,
            0x220031,
            0x220032,
            1,
            2,
            3,
            4,
            20,
            10,
        ),
    )
    .unwrap();
    assert_eq!(
        copy,
        XWireRequest::CopyArea {
            source: XResourceId::new(0x220030, 1),
            destination: XResourceId::new(0x220031, 1),
            gc: XResourceId::new(0x220032, 1),
            src_x: 1,
            src_y: 2,
            dst_x: 3,
            dst_y: 4,
            width: 20,
            height: 10,
        }
    );
}

#[test]
fn x11_core_decoder_captures_font_requests() {
    let namespace = NamespaceId::from_raw(45);
    let open = decode_x11_core_request(
        context(namespace, 573, XByteOrder::LittleEndian),
        &open_font_request(XByteOrder::LittleEndian, 0x220040, "fixed"),
    )
    .unwrap();
    assert_eq!(
        open,
        XWireRequest::OpenFont {
            font: XResourceId::new(0x220040, 1),
            name: "fixed".to_owned(),
        }
    );

    let close = decode_x11_core_request(
        context(namespace, 574, XByteOrder::LittleEndian),
        &resource_request(XByteOrder::LittleEndian, 47, 0x220040),
    )
    .unwrap();
    assert_eq!(
        close,
        XWireRequest::QueryFont {
            font: XResourceId::new(0x220040, 1),
        }
    );

    let close = decode_x11_core_request(
        context(namespace, 575, XByteOrder::LittleEndian),
        &resource_request(XByteOrder::LittleEndian, 46, 0x220040),
    )
    .unwrap();
    assert_eq!(
        close,
        XWireRequest::CloseFont {
            font: XResourceId::new(0x220040, 1),
        }
    );

    let list = decode_x11_core_request(
        context(namespace, 576, XByteOrder::LittleEndian),
        &list_fonts_request(XByteOrder::LittleEndian, 5, "*"),
    )
    .unwrap();
    assert_eq!(
        list,
        XWireRequest::ListFonts {
            max_names: 5,
            pattern: "*".to_owned(),
        }
    );

    let list = decode_x11_core_request(
        context(namespace, 577, XByteOrder::LittleEndian),
        &list_fonts_with_info_request(XByteOrder::LittleEndian, 5, "*"),
    )
    .unwrap();
    assert_eq!(
        list,
        XWireRequest::ListFontsWithInfo {
            max_names: 5,
            pattern: "*".to_owned(),
        }
    );

    let cursor = decode_x11_core_request(
        context(namespace, 578, XByteOrder::LittleEndian),
        &create_glyph_cursor_request(XByteOrder::LittleEndian, 0x220050, 0x220040, 0x220041),
    )
    .unwrap();
    assert_eq!(
        cursor,
        XWireRequest::CreateGlyphCursor {
            cursor: XResourceId::new(0x220050, 1),
            source_font: XResourceId::new(0x220040, 1),
            mask_font: Some(XResourceId::new(0x220041, 1)),
        }
    );

    let free_cursor = decode_x11_core_request(
        context(namespace, 579, XByteOrder::LittleEndian),
        &resource_request(XByteOrder::LittleEndian, 95, 0x220050),
    )
    .unwrap();
    assert_eq!(
        free_cursor,
        XWireRequest::FreeCursor {
            cursor: XResourceId::new(0x220050, 1),
        }
    );

    let recolor_cursor = decode_x11_core_request(
        context(namespace, 580, XByteOrder::LittleEndian),
        &recolor_cursor_request(XByteOrder::LittleEndian, 0x220050),
    )
    .unwrap();
    assert_eq!(
        recolor_cursor,
        XWireRequest::RecolorCursor {
            cursor: XResourceId::new(0x220050, 1),
        }
    );
}

#[test]
fn x11_core_decoder_captures_query_extension_requests() {
    let namespace = NamespaceId::from_raw(45);
    let query = decode_x11_core_request(
        context(namespace, 507, XByteOrder::LittleEndian),
        &query_extension_request(XByteOrder::LittleEndian, "BIG-REQUESTS"),
    )
    .unwrap();

    assert_eq!(
        query,
        XWireRequest::QueryExtension {
            name: "BIG-REQUESTS".to_owned(),
        }
    );
}

#[test]
fn x11_core_decoder_captures_sophia_present_pixmap_requests() {
    let namespace = NamespaceId::from_raw(45);
    let present = decode_x11_core_request(
        context(namespace, 509, XByteOrder::LittleEndian),
        &sophia_present_pixmap_request(
            XByteOrder::LittleEndian,
            0x220030,
            0x900,
            (4, 5, 64, 48),
            3,
            250,
        ),
    )
    .unwrap();

    assert_eq!(
        present,
        XWireRequest::Authority(XAuthorityRequestPacket {
            transaction: TransactionId::from_raw(509),
            namespace,
            kind: XAuthorityRequestKind::PresentPixmap {
                window: XResourceId::new(0x220030, 1),
                pixmap: 0x900,
                damage: Region::single(Rect {
                    x: 4,
                    y: 5,
                    width: 64,
                    height: 48,
                }),
                previous_committed_generation: 3,
                timeout_msec: 250,
            },
        })
    );
}

#[test]
fn x11_core_decoder_captures_mit_shm_requests() {
    let namespace = NamespaceId::from_raw(45);

    let query = decode_x11_core_request(
        context(namespace, 530, XByteOrder::LittleEndian),
        &mit_shm_query_version_request(XByteOrder::LittleEndian),
    )
    .unwrap();
    assert_eq!(query, XWireRequest::ShmQueryVersion);

    let attach = decode_x11_core_request(
        context(namespace, 531, XByteOrder::LittleEndian),
        &mit_shm_attach_request(XByteOrder::LittleEndian, 0x440001, 77, true),
    )
    .unwrap();
    assert_eq!(
        attach,
        XWireRequest::ShmAttach {
            segment: XResourceId::new(0x440001, 1),
            shmid: 77,
            read_only: true,
        }
    );

    let put = decode_x11_core_request(
        context(namespace, 532, XByteOrder::LittleEndian),
        &mit_shm_put_image_request(XByteOrder::LittleEndian, 0x220701, 0x220702, 0x440001, 128),
    )
    .unwrap();
    assert_eq!(
        put,
        XWireRequest::ShmPutImage {
            drawable: XResourceId::new(0x220701, 1),
            gc: XResourceId::new(0x220702, 1),
            total_width: 64,
            total_height: 48,
            src_x: 0,
            src_y: 0,
            src_width: 32,
            src_height: 24,
            dst_x: 3,
            dst_y: 5,
            depth: 24,
            format: 2,
            send_event: false,
            segment: XResourceId::new(0x440001, 1),
            offset: 128,
        }
    );
}

#[test]
fn x11_dispatch_reports_root_input_focus_for_minimal_server() {
    let namespace = NamespaceId::from_raw(45);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let request = decode_x11_core_request(
        context(namespace, 522, XByteOrder::LittleEndian),
        &[43, 0, 1, 0],
    )
    .unwrap();

    let result = dispatch_x11_wire_request(
        dispatch_context(namespace, 1, XByteOrder::LittleEndian, 43),
        request,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = result.encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(encoded[0][0], 1);
    assert_eq!(encoded[0][1], 1);
    assert_eq!(
        read_u32(XByteOrder::LittleEndian, &encoded[0][8..12]),
        X_SETUP_DEFAULT_ROOT
    );
}

#[test]
fn x11_dispatch_reports_core_modifier_mapping() {
    let namespace = NamespaceId::from_raw(45);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let request = decode_x11_core_request(
        context(namespace, 523, XByteOrder::LittleEndian),
        &[119, 0, 1, 0],
    )
    .unwrap();

    let result = dispatch_x11_wire_request(
        dispatch_context(namespace, 2, XByteOrder::LittleEndian, 119),
        request,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = result.encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(encoded.len(), 1);
    assert_eq!(encoded[0].len(), 48);
    assert_eq!(encoded[0][0], 1);
    assert_eq!(encoded[0][1], 2);
    assert_eq!(read_u16(XByteOrder::LittleEndian, &encoded[0][2..4]), 2);
    assert_eq!(read_u32(XByteOrder::LittleEndian, &encoded[0][4..8]), 4);
    assert_eq!(&encoded[0][32..36], &[50, 62, 66, 0]);
}

#[test]
fn x11_dispatch_reports_us_keyboard_mapping_for_minimal_server() {
    let namespace = NamespaceId::from_raw(45);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let request = decode_x11_core_request(
        context(namespace, 524, XByteOrder::LittleEndian),
        &[101, 0, 2, 0, 8, 4, 0, 0],
    )
    .unwrap();

    let result = dispatch_x11_wire_request(
        dispatch_context(namespace, 3, XByteOrder::LittleEndian, 101),
        request,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = result.encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(encoded.len(), 1);
    assert_eq!(encoded[0].len(), 64);
    assert_eq!(encoded[0][0], 1);
    assert_eq!(encoded[0][1], 2);
    assert_eq!(read_u16(XByteOrder::LittleEndian, &encoded[0][2..4]), 3);
    assert_eq!(read_u32(XByteOrder::LittleEndian, &encoded[0][4..8]), 8);
    let keysyms = encoded[0][32..64]
        .chunks_exact(4)
        .map(|bytes| read_u32(XByteOrder::LittleEndian, bytes))
        .collect::<Vec<_>>();
    assert_eq!(
        keysyms,
        vec![
            0,
            0,
            0xff1b,
            0xff1b,
            b'1' as u32,
            b'!' as u32,
            b'2' as u32,
            b'@' as u32
        ]
    );
}

#[test]
fn x11_dispatch_reports_evdev_navigation_keysyms() {
    let namespace = NamespaceId::from_raw(45);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let request = decode_x11_core_request(
        context(namespace, 525, XByteOrder::LittleEndian),
        &[101, 0, 2, 0, 111, 6, 0, 0],
    )
    .unwrap();

    let result = dispatch_x11_wire_request(
        dispatch_context(namespace, 4, XByteOrder::LittleEndian, 101),
        request,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = result.encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(encoded.len(), 1);
    assert_eq!(encoded[0][1], 2);
    let keysyms = encoded[0][32..]
        .chunks_exact(4)
        .map(|bytes| read_u32(XByteOrder::LittleEndian, bytes))
        .collect::<Vec<_>>();
    assert_eq!(
        keysyms,
        vec![
            0xff52, 0xff52, 0xff55, 0xff55, 0xff51, 0xff51, 0xff53, 0xff53, 0xff57, 0xff57, 0xff54,
            0xff54,
        ]
    );
}

#[test]
fn x11_dispatch_replies_to_atom_requests_and_rejects_unknown_names() {
    let namespace = NamespaceId::from_raw(45);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();

    let intern = decode_x11_core_request(
        context(namespace, 508, XByteOrder::LittleEndian),
        &intern_atom_request(XByteOrder::LittleEndian, false, X_ATOM_NAME_NET_WM_NAME),
    )
    .unwrap();
    let intern = dispatch_x11_wire_request(
        dispatch_context(namespace, 1, XByteOrder::LittleEndian, 16),
        intern,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = intern.encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(encoded.len(), 1);
    assert_eq!(encoded[0][0], 1);
    let net_wm_name = read_u32(XByteOrder::LittleEndian, &encoded[0][8..12]);
    assert_ne!(net_wm_name, 0);

    let missing = decode_x11_core_request(
        context(namespace, 509, XByteOrder::LittleEndian),
        &intern_atom_request(XByteOrder::LittleEndian, true, "SOPHIA_MISSING"),
    )
    .unwrap();
    let missing = dispatch_x11_wire_request(
        dispatch_context(namespace, 2, XByteOrder::LittleEndian, 16),
        missing,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = missing.encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(read_u32(XByteOrder::LittleEndian, &encoded[0][8..12]), 0);

    let get_name = decode_x11_core_request(
        context(namespace, 510, XByteOrder::LittleEndian),
        &get_atom_name_request(XByteOrder::LittleEndian, net_wm_name),
    )
    .unwrap();
    let get_name = dispatch_x11_wire_request(
        dispatch_context(namespace, 3, XByteOrder::LittleEndian, 17),
        get_name,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = get_name.encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(read_u16(XByteOrder::LittleEndian, &encoded[0][8..10]), 12);
    assert_eq!(&encoded[0][32..44], X_ATOM_NAME_NET_WM_NAME.as_bytes());

    let unknown = decode_x11_core_request(
        context(namespace, 511, XByteOrder::LittleEndian),
        &get_atom_name_request(XByteOrder::LittleEndian, 0x00ff_ffff),
    )
    .unwrap();
    let unknown = dispatch_x11_wire_request(
        dispatch_context(namespace, 4, XByteOrder::LittleEndian, 17),
        unknown,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = unknown.encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(encoded[0][0], 0);
    assert_eq!(encoded[0][1], XErrorCode::BadAtom.wire_code());
}

#[test]
fn x11_dispatch_reports_extensions_absent_until_explicitly_supported() {
    let namespace = NamespaceId::from_raw(45);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let query = decode_x11_core_request(
        context(namespace, 521, XByteOrder::LittleEndian),
        &query_extension_request(XByteOrder::LittleEndian, "SOPHIA-UNKNOWN"),
    )
    .unwrap();

    let result = dispatch_x11_wire_request(
        dispatch_context(namespace, 1, XByteOrder::LittleEndian, 98),
        query,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = result.encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(encoded[0][0], 1);
    assert_eq!(encoded[0][8], 0);
    assert_eq!(encoded[0][9], 0);
}

#[test]
fn x11_dispatch_advertises_sophia_present_extension() {
    let namespace = NamespaceId::from_raw(45);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let query = decode_x11_core_request(
        context(namespace, 524, XByteOrder::LittleEndian),
        &query_extension_request(XByteOrder::LittleEndian, X_SOPHIA_PRESENT_EXTENSION_NAME),
    )
    .unwrap();

    let result = dispatch_x11_wire_request(
        dispatch_context(namespace, 1, XByteOrder::LittleEndian, 98),
        query,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = result.encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(encoded[0][0], 1);
    assert_eq!(encoded[0][8], 1);
    assert_eq!(encoded[0][9], X_SOPHIA_PRESENT_MAJOR_OPCODE);
    assert_eq!(encoded[0][10], 0);
    assert_eq!(encoded[0][11], 0);
}

#[test]
fn x11_dispatch_advertises_mit_shm_and_replies_to_query_version() {
    let namespace = NamespaceId::from_raw(45);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let query = decode_x11_core_request(
        context(namespace, 526, XByteOrder::LittleEndian),
        &query_extension_request(XByteOrder::LittleEndian, X_MIT_SHM_EXTENSION_NAME),
    )
    .unwrap();

    let result = dispatch_x11_wire_request(
        dispatch_context(namespace, 1, XByteOrder::LittleEndian, 98),
        query,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = result.encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(encoded[0][0], 1);
    assert_eq!(encoded[0][8], 1);
    assert_eq!(encoded[0][9], X_MIT_SHM_MAJOR_OPCODE);

    let version = decode_x11_core_request(
        context(namespace, 527, XByteOrder::LittleEndian),
        &mit_shm_query_version_request(XByteOrder::LittleEndian),
    )
    .unwrap();
    let version = dispatch_x11_wire_request(
        dispatch_context(
            namespace,
            2,
            XByteOrder::LittleEndian,
            X_MIT_SHM_MAJOR_OPCODE,
        ),
        version,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = version.encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(encoded[0][0], 1);
    assert_eq!(encoded[0][1], 0);
    assert_eq!(read_u16(XByteOrder::LittleEndian, &encoded[0][8..10]), 1);
    assert_eq!(read_u16(XByteOrder::LittleEndian, &encoded[0][10..12]), 2);
}

#[test]
fn x11_dispatch_negotiates_standard_dri3_and_present_1_2() {
    let namespace = NamespaceId::from_raw(45);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    for (name, opcode, first_event) in [
        (X_DRI3_EXTENSION_NAME, X_DRI3_MAJOR_OPCODE, 0),
        (
            X_PRESENT_EXTENSION_NAME,
            X_PRESENT_MAJOR_OPCODE,
            X_PRESENT_FIRST_EVENT,
        ),
    ] {
        let query = decode_x11_core_request(
            context(namespace, 528, XByteOrder::LittleEndian),
            &query_extension_request(XByteOrder::LittleEndian, name),
        )
        .unwrap();
        let result = dispatch_x11_wire_request(
            dispatch_context(namespace, 1, XByteOrder::LittleEndian, 98),
            query,
            &mut runtime,
            &mut atoms,
            &mut properties,
        );
        let encoded = result.encoded_outputs(XByteOrder::LittleEndian);
        assert_eq!(encoded[0][8], 1);
        assert_eq!(encoded[0][9], opcode);
        assert_eq!(encoded[0][10], first_event);

        let version = decode_x11_core_request(
            context(namespace, 529, XByteOrder::LittleEndian),
            &extension_query_version_request(XByteOrder::LittleEndian, opcode, 1, 4),
        )
        .unwrap();
        let version = dispatch_x11_wire_request(
            dispatch_context(namespace, 2, XByteOrder::LittleEndian, opcode),
            version,
            &mut runtime,
            &mut atoms,
            &mut properties,
        );
        let encoded = version.encoded_outputs(XByteOrder::LittleEndian);
        assert_eq!(read_u32(XByteOrder::LittleEndian, &encoded[0][8..12]), 1);
        assert_eq!(read_u32(XByteOrder::LittleEndian, &encoded[0][12..16]), 2);
    }
}

#[test]
fn dri3_open_decodes_default_provider_and_encodes_one_fd_reply() {
    let namespace = NamespaceId::from_raw(45);
    let request = decode_x11_core_request(
        context(namespace, 529, XByteOrder::LittleEndian),
        &dri3_open_request(XByteOrder::LittleEndian, X_SETUP_DEFAULT_ROOT, 0),
    )
    .unwrap();
    assert_eq!(request.required_fd_count(), 0);
    assert_eq!(
        request,
        XWireRequest::Dri3Open {
            drawable: XResourceId::new(u64::from(X_SETUP_DEFAULT_ROOT), 1),
            provider: 0,
        }
    );

    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let result = dispatch_x11_wire_request(
        dispatch_context(namespace, 7, XByteOrder::LittleEndian, X_DRI3_MAJOR_OPCODE),
        request,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = result.encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(encoded.len(), 1);
    assert_eq!(encoded[0][0], 1);
    assert_eq!(encoded[0][1], 1);
    assert_eq!(read_u16(XByteOrder::LittleEndian, &encoded[0][2..4]), 7);
    assert_eq!(read_u32(XByteOrder::LittleEndian, &encoded[0][4..8]), 0);
}

#[test]
fn dri3_open_rejects_nondefault_provider() {
    let namespace = NamespaceId::from_raw(45);
    let request = decode_x11_core_request(
        context(namespace, 529, XByteOrder::LittleEndian),
        &dri3_open_request(XByteOrder::LittleEndian, X_SETUP_DEFAULT_ROOT, 99),
    )
    .unwrap();
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let result = dispatch_x11_wire_request(
        dispatch_context(namespace, 8, XByteOrder::LittleEndian, X_DRI3_MAJOR_OPCODE),
        request,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = result.encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(encoded[0][0], 0);
    assert_eq!(encoded[0][1], XErrorCode::BadValue.wire_code());
    assert_eq!(read_u32(XByteOrder::LittleEndian, &encoded[0][4..8]), 99);
    assert_eq!(
        read_u16(XByteOrder::LittleEndian, &encoded[0][8..10]),
        u16::from(X_DRI3_OPEN_MINOR_OPCODE)
    );
}

#[test]
fn dri3_get_supported_modifiers_reports_linear_and_implicit_screen_layouts() {
    let namespace = NamespaceId::from_raw(45);
    let request = decode_x11_core_request(
        context(namespace, 529, XByteOrder::LittleEndian),
        &dri3_get_supported_modifiers_request(
            XByteOrder::LittleEndian,
            X_SETUP_DEFAULT_ROOT,
            24,
            32,
        ),
    )
    .unwrap();
    assert_eq!(
        request,
        XWireRequest::Dri3GetSupportedModifiers {
            window: XResourceId::new(u64::from(X_SETUP_DEFAULT_ROOT), 1),
            depth: 24,
            bits_per_pixel: 32,
        }
    );
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let result = dispatch_x11_wire_request(
        dispatch_context(namespace, 9, XByteOrder::LittleEndian, X_DRI3_MAJOR_OPCODE),
        request,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = result.encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(encoded[0].len(), 48);
    assert_eq!(read_u32(XByteOrder::LittleEndian, &encoded[0][4..8]), 4);
    assert_eq!(read_u32(XByteOrder::LittleEndian, &encoded[0][8..12]), 0);
    assert_eq!(read_u32(XByteOrder::LittleEndian, &encoded[0][12..16]), 2);
    assert_eq!(read_u64(XByteOrder::LittleEndian, &encoded[0][32..40]), 0);
    assert_eq!(
        read_u64(XByteOrder::LittleEndian, &encoded[0][40..48]),
        0x00ff_ffff_ffff_ffff
    );

    let invalid = decode_x11_core_request(
        context(namespace, 530, XByteOrder::LittleEndian),
        &dri3_get_supported_modifiers_request(
            XByteOrder::LittleEndian,
            X_SETUP_DEFAULT_ROOT,
            16,
            16,
        ),
    )
    .unwrap();
    let invalid = dispatch_x11_wire_request(
        dispatch_context(namespace, 10, XByteOrder::LittleEndian, X_DRI3_MAJOR_OPCODE),
        invalid,
        &mut runtime,
        &mut atoms,
        &mut properties,
    )
    .encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(invalid[0][0], 0);
    assert_eq!(invalid[0][1], XErrorCode::BadValue.wire_code());
}

#[test]
fn dri3_pixmap_from_buffer_requires_one_fd_and_preserves_bounded_metadata() {
    let namespace = NamespaceId::from_raw(45);
    let request = decode_x11_core_request(
        context(namespace, 530, XByteOrder::LittleEndian),
        &dri3_pixmap_from_buffer_request(
            XByteOrder::LittleEndian,
            0x220801,
            X_SETUP_DEFAULT_ROOT,
            64 * 48 * 4,
            64,
            48,
            256,
            24,
            32,
        ),
    )
    .unwrap();
    assert_eq!(request.required_fd_count(), 1);
    assert_eq!(
        request,
        XWireRequest::Dri3PixmapFromBuffer {
            pixmap: XResourceId::new(0x220801, 1),
            drawable: XResourceId::new(u64::from(X_SETUP_DEFAULT_ROOT), 1),
            size_bytes: 64 * 48 * 4,
            width: 64,
            height: 48,
            stride: 256,
            depth: 24,
            bits_per_pixel: 32,
        }
    );
}

#[test]
fn dri3_pixmap_from_buffers_preserves_modifier_and_plane_metadata() {
    let namespace = NamespaceId::from_raw(45);
    let pixmap = XResourceId::new(0x220803, 1);
    let request = decode_x11_core_request(
        context(namespace, 531, XByteOrder::LittleEndian),
        &dri3_pixmap_from_buffers_request(
            XByteOrder::LittleEndian,
            0x220803,
            X_SETUP_DEFAULT_ROOT,
            1,
            64,
            48,
            [256, 0, 0, 0],
            [0, 0, 0, 0],
            24,
            32,
            0,
        ),
    )
    .unwrap();
    assert_eq!(request.required_fd_count(), 1);
    assert_eq!(
        request,
        XWireRequest::Dri3PixmapFromBuffers {
            pixmap,
            window: XResourceId::new(u64::from(X_SETUP_DEFAULT_ROOT), 1),
            num_buffers: 1,
            width: 64,
            height: 48,
            strides: [256, 0, 0, 0],
            offsets: [0, 0, 0, 0],
            depth: 24,
            bits_per_pixel: 32,
            modifier: 0,
        }
    );

    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let result = dispatch_x11_wire_request(
        dispatch_context(namespace, 3, XByteOrder::LittleEndian, X_DRI3_MAJOR_OPCODE),
        request,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    assert!(result.outputs.is_empty());
    let descriptor = runtime.dri3_pixmap_descriptor(namespace, pixmap).unwrap();
    assert_eq!(
        descriptor.size,
        Size {
            width: 64,
            height: 48,
        }
    );
    assert_eq!(descriptor.format, sophia_protocol::DRM_FORMAT_XRGB8888);
    assert_eq!(descriptor.modifier, 0);
    assert_eq!(descriptor.plane_count, 1);
    assert_eq!(descriptor.planes[0].unwrap().stride, 256);
    assert_eq!(descriptor.planes[0].unwrap().offset, 0);
}

#[test]
fn dri3_fence_from_fd_requires_one_fd_and_registers_authority_identity() {
    let namespace = NamespaceId::from_raw(45);
    let request = decode_x11_core_request(
        context(namespace, 531, XByteOrder::LittleEndian),
        &dri3_fence_from_fd_request(
            XByteOrder::LittleEndian,
            X_SETUP_DEFAULT_ROOT,
            0x220802,
            false,
        ),
    )
    .unwrap();
    assert_eq!(request.required_fd_count(), 1);
    assert_eq!(
        request,
        XWireRequest::Dri3FenceFromFd {
            drawable: XResourceId::new(u64::from(X_SETUP_DEFAULT_ROOT), 1),
            fence: XResourceId::new(0x220802, 1),
            initially_triggered: false,
        }
    );

    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let result = dispatch_x11_wire_request(
        dispatch_context(namespace, 3, XByteOrder::LittleEndian, X_DRI3_MAJOR_OPCODE),
        request,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    assert!(result.outputs.is_empty());
    runtime
        .validate_dri3_fence_access(namespace, XResourceId::new(0x220802, 1))
        .unwrap();
}

#[test]
fn standard_present_pixmap_reduces_dri3_pixmap_to_dmabuf_transaction() {
    let namespace = NamespaceId::from_raw(45);
    let window = XResourceId::new(0x220810, 1);
    let pixmap = XResourceId::new(0x220811, 1);
    let mut runtime = XAuthorityRuntime::new();
    runtime.apply(XAuthorityRequestPacket {
        transaction: TransactionId::from_raw(1),
        namespace,
        kind: XAuthorityRequestKind::CreateWindow {
            window,
            surface: SurfaceId::new(45, 1),
            geometry: Rect {
                x: 0,
                y: 0,
                width: 64,
                height: 48,
            },
            constraints: SurfaceConstraints {
                min_size: None,
                max_size: None,
            },
            generation: 1,
        },
    });
    let descriptor = runtime
        .create_dri3_pixmap(namespace, pixmap, 2, 64 * 48 * 4, 64, 48, 256, 24, 32)
        .unwrap();

    let request = decode_x11_core_request(
        context(namespace, 532, XByteOrder::LittleEndian),
        &present_pixmap_request(XByteOrder::LittleEndian, window, pixmap, 77),
    )
    .unwrap();
    assert_eq!(request.required_fd_count(), 0);
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let result = dispatch_x11_wire_request(
        dispatch_context(
            namespace,
            2,
            XByteOrder::LittleEndian,
            X_PRESENT_MAJOR_OPCODE,
        ),
        request,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    assert!(result.outputs.is_empty());
    let response = result.response.unwrap();
    assert_eq!(response.transactions.len(), 1);
    assert_eq!(
        response.transactions[0].target_buffer,
        BufferSource::DmaBuf {
            handle: descriptor.handle.raw()
        }
    );
    assert_eq!(response.transactions[0].damage.rects[0].width, 64);
    assert_eq!(response.transactions[0].damage.rects[0].height, 48);
}

#[test]
fn x11_dispatch_advertises_randr_and_replies_to_query_version() {
    let namespace = NamespaceId::from_raw(45);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let query = decode_x11_core_request(
        context(namespace, 538, XByteOrder::LittleEndian),
        &query_extension_request(XByteOrder::LittleEndian, X_RANDR_EXTENSION_NAME),
    )
    .unwrap();

    let result = dispatch_x11_wire_request(
        dispatch_context(namespace, 1, XByteOrder::LittleEndian, 98),
        query,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = result.encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(encoded[0][0], 1);
    assert_eq!(encoded[0][8], 1);
    assert_eq!(encoded[0][9], X_RANDR_MAJOR_OPCODE);

    let version = decode_x11_core_request(
        context(namespace, 539, XByteOrder::LittleEndian),
        &randr_query_version_request(XByteOrder::LittleEndian, 1, 5),
    )
    .unwrap();
    assert_eq!(
        version,
        XWireRequest::RandrQueryVersion {
            major_version: 1,
            minor_version: 5,
        }
    );
    let version = dispatch_x11_wire_request(
        dispatch_context(namespace, 2, XByteOrder::LittleEndian, X_RANDR_MAJOR_OPCODE),
        version,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = version.encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(encoded[0][0], 1);
    assert_eq!(read_u32(XByteOrder::LittleEndian, &encoded[0][8..12]), 1);
    assert_eq!(read_u32(XByteOrder::LittleEndian, &encoded[0][12..16]), 5);

    let select = decode_x11_core_request(
        context(namespace, 540, XByteOrder::LittleEndian),
        &randr_select_input_request(XByteOrder::LittleEndian, X_SETUP_DEFAULT_ROOT, 0x000b),
    )
    .unwrap();
    assert_eq!(
        select,
        XWireRequest::RandrSelectInput {
            window: XResourceId::new(u64::from(X_SETUP_DEFAULT_ROOT), 1),
            enable: 0x000b,
        }
    );
    let select = dispatch_x11_wire_request(
        dispatch_context(namespace, 3, XByteOrder::LittleEndian, X_RANDR_MAJOR_OPCODE),
        select,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    assert!(select.outputs.is_empty());

    let primary = decode_x11_core_request(
        context(namespace, 541, XByteOrder::LittleEndian),
        &randr_window_request(
            XByteOrder::LittleEndian,
            X_RANDR_GET_OUTPUT_PRIMARY_MINOR_OPCODE,
            X_SETUP_DEFAULT_ROOT,
        ),
    )
    .unwrap();
    assert_eq!(
        primary,
        XWireRequest::RandrGetOutputPrimary {
            window: XResourceId::new(u64::from(X_SETUP_DEFAULT_ROOT), 1),
        }
    );
    let primary = dispatch_x11_wire_request(
        dispatch_context(namespace, 4, XByteOrder::LittleEndian, X_RANDR_MAJOR_OPCODE),
        primary,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = primary.encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(encoded[0][0], 1);
    assert_eq!(
        read_u32(XByteOrder::LittleEndian, &encoded[0][8..12]),
        0x2000_0001
    );

    let monitors = decode_x11_core_request(
        context(namespace, 542, XByteOrder::LittleEndian),
        &randr_get_monitors_request(XByteOrder::LittleEndian, X_SETUP_DEFAULT_ROOT, true),
    )
    .unwrap();
    assert_eq!(
        monitors,
        XWireRequest::RandrGetMonitors {
            window: XResourceId::new(u64::from(X_SETUP_DEFAULT_ROOT), 1),
            get_active: true,
        }
    );
    let monitors = dispatch_x11_wire_request(
        dispatch_context(namespace, 5, XByteOrder::LittleEndian, X_RANDR_MAJOR_OPCODE),
        monitors,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = monitors.encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(encoded[0][0], 1);
    assert_eq!(read_u32(XByteOrder::LittleEndian, &encoded[0][12..16]), 1);
    assert_eq!(read_u32(XByteOrder::LittleEndian, &encoded[0][16..20]), 1);
    assert_eq!(read_u32(XByteOrder::LittleEndian, &encoded[0][4..8]), 7);
    assert_eq!(encoded[0][36], 1, "the deterministic monitor is primary");
    assert_eq!(read_u16(XByteOrder::LittleEndian, &encoded[0][38..40]), 1);
    assert_eq!(
        read_u16(XByteOrder::LittleEndian, &encoded[0][44..46]),
        1280
    );
    assert_eq!(read_u16(XByteOrder::LittleEndian, &encoded[0][46..48]), 720);
    assert_eq!(
        read_u32(XByteOrder::LittleEndian, &encoded[0][56..60]),
        0x2000_0001
    );
}

#[test]
fn randr_output_property_returns_bounded_empty_edid_fallback() {
    let namespace = NamespaceId::from_raw(45);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let edid = atoms.intern("EDID", false).unwrap().unwrap();
    let request = decode_x11_core_request(
        context(namespace, 543, XByteOrder::LittleEndian),
        &randr_get_output_property_request(XByteOrder::LittleEndian, 0x2000_0001, edid, 128),
    )
    .unwrap();
    assert!(matches!(
        request,
        XWireRequest::RandrGetOutputProperty {
            output: 0x2000_0001,
            property,
            long_length: 128,
            ..
        } if property == edid
    ));
    let result = dispatch_x11_wire_request(
        dispatch_context(namespace, 6, XByteOrder::LittleEndian, X_RANDR_MAJOR_OPCODE),
        request,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = result.encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(encoded[0].len(), 32);
    assert_eq!(encoded[0][0], 1);
    assert_eq!(encoded[0][1], 0);
    assert_eq!(read_u32(XByteOrder::LittleEndian, &encoded[0][4..8]), 0);
    assert_eq!(read_u32(XByteOrder::LittleEndian, &encoded[0][8..12]), 0);
    assert_eq!(read_u32(XByteOrder::LittleEndian, &encoded[0][12..16]), 0);
    assert_eq!(read_u32(XByteOrder::LittleEndian, &encoded[0][16..20]), 0);
}

#[test]
fn xfixes_regions_support_create_set_and_destroy_lifecycle() {
    let namespace = NamespaceId::from_raw(45);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let region = 0x220100;
    let rectangles = [Rect {
        x: 0,
        y: 0,
        width: 310,
        height: 257,
    }];

    for (sequence, request) in [
        xfixes_create_region_request(XByteOrder::LittleEndian, region, &[]),
        xfixes_set_region_request(XByteOrder::LittleEndian, region, &rectangles),
    ]
    .into_iter()
    .enumerate()
    {
        let request = decode_x11_core_request(
            context(namespace, 540 + sequence as u64, XByteOrder::LittleEndian),
            &request,
        )
        .unwrap();
        if sequence == 1 {
            assert!(matches!(
                request,
                XWireRequest::XfixesSetRegion {
                    rectangles: ref decoded,
                    ..
                } if decoded == &rectangles
            ));
        }
        let result = dispatch_x11_wire_request(
            dispatch_context(
                namespace,
                5 + sequence as u16,
                XByteOrder::LittleEndian,
                X_XFIXES_MAJOR_OPCODE,
            ),
            request,
            &mut runtime,
            &mut atoms,
            &mut properties,
        );
        assert!(result.outputs.is_empty());
    }

    let region_id = XResourceId::new(u64::from(region), 1);
    assert_eq!(
        runtime.validate_xfixes_region_access(namespace, region_id),
        Ok(())
    );
    let destroy = XWireRequest::XfixesDestroyRegion { region: region_id };
    let result = dispatch_x11_wire_request(
        dispatch_context(
            namespace,
            7,
            XByteOrder::LittleEndian,
            X_XFIXES_MAJOR_OPCODE,
        ),
        destroy,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    assert!(result.outputs.is_empty());
    assert_eq!(
        runtime.validate_xfixes_region_access(namespace, region_id),
        Err(XAuthorityRuntimeError::UnknownResource)
    );
}

#[test]
fn xfixes_selection_subscription_accepts_known_window_atom_and_mask() {
    let namespace = NamespaceId::from_raw(45);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let window = 0x220101;
    let create = decode_x11_core_request(
        context(namespace, 543, XByteOrder::LittleEndian),
        &create_window_request(XByteOrder::LittleEndian, window, 0, 0, 1, 1),
    )
    .unwrap();
    dispatch_x11_wire_request(
        dispatch_context(namespace, 6, XByteOrder::LittleEndian, 1),
        create,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let request = decode_x11_core_request(
        context(namespace, 544, XByteOrder::LittleEndian),
        &xfixes_select_selection_input_request(
            XByteOrder::LittleEndian,
            window,
            X_ATOM_PRIMARY,
            0b111,
        ),
    )
    .unwrap();
    assert!(matches!(
        request,
        XWireRequest::XfixesSelectSelectionInput {
            selection: X_ATOM_PRIMARY,
            event_mask: 0b111,
            ..
        }
    ));
    let result = dispatch_x11_wire_request(
        dispatch_context(
            namespace,
            7,
            XByteOrder::LittleEndian,
            X_XFIXES_MAJOR_OPCODE,
        ),
        request,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    assert!(result.outputs.is_empty());
}

#[test]
fn x11_dispatch_advertises_probe_backed_xkeyboard_extension() {
    let namespace = NamespaceId::from_raw(45);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let query = decode_x11_core_request(
        context(namespace, 545, XByteOrder::LittleEndian),
        &query_extension_request(XByteOrder::LittleEndian, X_KEYBOARD_EXTENSION_NAME),
    )
    .unwrap();

    let result = dispatch_x11_wire_request(
        dispatch_context(namespace, 1, XByteOrder::LittleEndian, 98),
        query,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = result.encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(encoded[0][0], 1);
    assert_eq!(encoded[0][8], 1);
    assert_eq!(encoded[0][9], X_KEYBOARD_MAJOR_OPCODE);
    assert_eq!(encoded[0][10], X_KEYBOARD_FIRST_EVENT);

    let use_extension = decode_x11_core_request(
        context(namespace, 546, XByteOrder::LittleEndian),
        &xkb_use_extension_request(XByteOrder::LittleEndian, 1, 0),
    )
    .unwrap();
    assert_eq!(
        use_extension,
        XWireRequest::XkbUseExtension {
            wanted_major: 1,
            wanted_minor: 0,
        }
    );
    let use_extension = dispatch_x11_wire_request(
        dispatch_context(
            namespace,
            2,
            XByteOrder::LittleEndian,
            X_KEYBOARD_MAJOR_OPCODE,
        ),
        use_extension,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = use_extension.encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(encoded[0][0], 1);
    assert_eq!(encoded[0][1], 1);
    assert_eq!(read_u16(XByteOrder::LittleEndian, &encoded[0][8..10]), 1);
    assert_eq!(read_u16(XByteOrder::LittleEndian, &encoded[0][10..12]), 0);
}

#[test]
fn xkb_state_names_and_state_subscription_use_standard_wire_layouts() {
    let namespace = NamespaceId::from_raw(45);
    let order = XByteOrder::LittleEndian;
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();

    let get_state = decode_x11_core_request(
        context(namespace, 1, order),
        &[
            X_KEYBOARD_MAJOR_OPCODE,
            X_KEYBOARD_GET_STATE_MINOR_OPCODE,
            2,
            0,
            3,
            0,
            0,
            0,
        ],
    )
    .unwrap();
    assert_eq!(get_state, XWireRequest::XkbGetState);
    let state = dispatch_x11_wire_request(
        dispatch_context(namespace, 1, order, X_KEYBOARD_MAJOR_OPCODE),
        get_state,
        &mut runtime,
        &mut atoms,
        &mut properties,
    )
    .encoded_outputs(order);
    assert_eq!(state[0].len(), 32);
    assert_eq!(state[0][1], 3);

    let names = decode_x11_core_request(
        context(namespace, 2, order),
        &[
            X_KEYBOARD_MAJOR_OPCODE,
            X_KEYBOARD_GET_NAMES_MINOR_OPCODE,
            3,
            0,
            3,
            0,
            0,
            0,
            0x3f,
            0,
            0,
            0,
        ],
    )
    .unwrap();
    assert_eq!(names, XWireRequest::XkbGetNames { which: 0x3f });
    let names = dispatch_x11_wire_request(
        dispatch_context(namespace, 2, order, X_KEYBOARD_MAJOR_OPCODE),
        names,
        &mut runtime,
        &mut atoms,
        &mut properties,
    )
    .encoded_outputs(order);
    assert_eq!(read_u32(order, &names[0][8..12]), 0x3f);
    assert_eq!(names[0].len(), 56);
    assert_eq!(names[0][12], 8);
    assert_eq!(names[0][13], u8::MAX);

    let select = decode_x11_core_request(
        context(namespace, 3, order),
        &[
            X_KEYBOARD_MAJOR_OPCODE,
            X_KEYBOARD_SELECT_EVENTS_MINOR_OPCODE,
            5,
            0,
            3,
            0,
            4,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            1,
            0,
            1,
            0,
        ],
    )
    .unwrap();
    assert_eq!(
        select,
        XWireRequest::XkbSelectEvents {
            affect_which: 4,
            clear: 0,
            select_all: 0,
            state_details: Some((1, 1)),
        }
    );

    let notify = encode_x_client_event(
        order,
        XClientEvent::XkbStateNotify {
            sequence: 7,
            time: 11,
            modifiers: 1,
            changed: 1,
            keycode: 50,
            event_type: 2,
        },
    );
    assert_eq!(notify[0], X_KEYBOARD_FIRST_EVENT);
    assert_eq!(notify[1], 2);
    assert_eq!(read_u16(order, &notify[24..26]), 1);
    assert_eq!(notify[26], 50);
}

#[test]
fn xge_and_xi2_report_versioned_master_device_classes() {
    let namespace = NamespaceId::from_raw(45);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let query = decode_x11_core_request(
        context(namespace, 1, XByteOrder::LittleEndian),
        &query_extension_request(XByteOrder::LittleEndian, X_GENERIC_EVENT_EXTENSION_NAME),
    )
    .unwrap();
    let encoded = dispatch_x11_wire_request(
        dispatch_context(namespace, 1, XByteOrder::LittleEndian, 98),
        query,
        &mut runtime,
        &mut atoms,
        &mut properties,
    )
    .encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(encoded[0][8], 1);
    assert_eq!(encoded[0][9], X_GENERIC_EVENT_MAJOR_OPCODE);

    let version = decode_x11_core_request(
        context(namespace, 2, XByteOrder::LittleEndian),
        &[X_GENERIC_EVENT_MAJOR_OPCODE, 0, 2, 0, 1, 0, 0, 0],
    )
    .unwrap();
    assert_eq!(
        version,
        XWireRequest::GeQueryVersion {
            major_version: 1,
            minor_version: 0
        }
    );
    let encoded = dispatch_x11_wire_request(
        dispatch_context(
            namespace,
            2,
            XByteOrder::LittleEndian,
            X_GENERIC_EVENT_MAJOR_OPCODE,
        ),
        version,
        &mut runtime,
        &mut atoms,
        &mut properties,
    )
    .encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(read_u16(XByteOrder::LittleEndian, &encoded[0][8..10]), 1);

    let devices = dispatch_x11_wire_request(
        dispatch_context(namespace, 3, XByteOrder::LittleEndian, X_INPUT_MAJOR_OPCODE),
        XWireRequest::XiQueryDevice { device_id: 0 },
        &mut runtime,
        &mut atoms,
        &mut properties,
    )
    .encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(read_u16(XByteOrder::LittleEndian, &devices[0][8..10]), 2);
    assert_eq!(read_u16(XByteOrder::LittleEndian, &devices[0][38..40]), 3);
    assert!(devices[0].len() > 128);
}

#[test]
fn xkb_get_map_encodes_schema_aligned_types_symbols_and_modifier_map() {
    let namespace = NamespaceId::from_raw(45);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let result = dispatch_x11_wire_request(
        dispatch_context(
            namespace,
            4,
            XByteOrder::LittleEndian,
            X_KEYBOARD_MAJOR_OPCODE,
        ),
        XWireRequest::XkbGetMap {
            full: 0x47,
            partial: 0,
        },
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = result.encoded_outputs(XByteOrder::LittleEndian);
    let reply = &encoded[0];
    assert_eq!(&reply[8..10], &[0, 0]);
    assert_eq!(reply[10], 8);
    assert_eq!(reply[11], u8::MAX);
    assert_eq!(read_u16(XByteOrder::LittleEndian, &reply[12..14]), 0x47);
    assert_eq!(&reply[14..18], &[0, 1, 1, 8]);
    assert_eq!(read_u16(XByteOrder::LittleEndian, &reply[18..20]), 496);
    assert_eq!(reply[20], 248);
    assert_eq!(&reply[31..34], &[8, 248, 10]);
    assert_eq!(
        read_u32(XByteOrder::LittleEndian, &reply[4..8]) as usize,
        (reply.len() - 32) / 4
    );
    assert_eq!(&reply[40..48], &[1, 1, 0, 0, 2, 1, 0, 0]);
    assert_eq!(&reply[56..64], &[0, 0, 0, 0, 1, 2, 2, 0]);
}

#[test]
fn xkb_state_uses_deterministic_rmlvo_and_tracks_effective_modifiers() {
    let mut keyboard = XkbKeyboardState::new(&XkbRmlvoConfig::default()).unwrap();
    assert_eq!(keyboard.map_evdev_key(42, true), Some((50, 0)));
    assert_eq!(keyboard.map_evdev_key(30, true), Some((38, 1)));
    assert_eq!(keyboard.map_evdev_key(30, false), Some((38, 1)));
    assert_eq!(keyboard.map_evdev_key(42, false), Some((50, 1)));
    assert_eq!(keyboard.modifier_mask(), 0);
}

#[test]
fn xkb_snapshot_drives_core_and_xkb_maps_from_the_same_rmlvo() {
    let us = XkbKeymapSnapshot::new(&XkbRmlvoConfig::default()).unwrap();
    let mut de_config = XkbRmlvoConfig::default();
    de_config.layout = "de".to_owned();
    let de = XkbKeymapSnapshot::new(&de_config).unwrap();

    assert_eq!(us.config().layout, "us");
    assert_eq!(de.config().layout, "de");
    assert_eq!(us.core_mapping(8, 248), us.xkb_keysyms().concat());
    assert_eq!(de.core_mapping(8, 248), de.xkb_keysyms().concat());
    assert_ne!(us.core_mapping(29, 1), de.core_mapping(29, 1));
}

#[test]
fn xkb_rmlvo_validation_rejects_empty_and_unbounded_configuration() {
    let mut empty = XkbRmlvoConfig::default();
    empty.layout.clear();
    assert_eq!(
        XkbKeyboardState::new(&empty).unwrap_err(),
        XkbKeyboardError::InvalidConfiguration
    );

    let mut unbounded = XkbRmlvoConfig::default();
    unbounded.options = "x".repeat(XKB_RMLVO_FIELD_MAX_BYTES + 1);
    assert_eq!(
        XkbKeyboardState::new(&unbounded).unwrap_err(),
        XkbKeyboardError::InvalidConfiguration
    );
}

#[test]
fn input_focus_state_is_scoped_per_namespace() {
    let first = NamespaceId::from_raw(45);
    let second = NamespaceId::from_raw(46);
    let mut runtime = XAuthorityRuntime::new();
    let none = XResourceId::new(0, 1);

    runtime.set_input_focus(first, none, 2).unwrap();
    assert_eq!(runtime.input_focus(first), (none, 2));
    assert_eq!(
        runtime.input_focus(second),
        (XResourceId::new(u64::from(X_SETUP_DEFAULT_ROOT), 1), 1)
    );
}

#[test]
fn x11_dispatch_advertises_big_requests_and_replies_to_enable() {
    let namespace = NamespaceId::from_raw(45);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let query = decode_x11_core_request(
        context(namespace, 547, XByteOrder::LittleEndian),
        &query_extension_request(XByteOrder::LittleEndian, X_BIG_REQUESTS_EXTENSION_NAME),
    )
    .unwrap();

    let result = dispatch_x11_wire_request(
        dispatch_context(namespace, 1, XByteOrder::LittleEndian, 98),
        query,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = result.encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(encoded[0][0], 1);
    assert_eq!(encoded[0][8], 1);
    assert_eq!(encoded[0][9], X_BIG_REQUESTS_MAJOR_OPCODE);

    let enable = decode_x11_core_request(
        context(namespace, 548, XByteOrder::LittleEndian),
        &[
            X_BIG_REQUESTS_MAJOR_OPCODE,
            X_BIG_REQUESTS_ENABLE_MINOR_OPCODE,
            1,
            0,
        ],
    )
    .unwrap();
    assert_eq!(enable, XWireRequest::BigRequestsEnable);
    let enable = dispatch_x11_wire_request(
        dispatch_context(
            namespace,
            2,
            XByteOrder::LittleEndian,
            X_BIG_REQUESTS_MAJOR_OPCODE,
        ),
        enable,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = enable.encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(encoded[0][0], 1);
    assert_eq!(read_u32(XByteOrder::LittleEndian, &encoded[0][8..12]), 4096);
}

#[test]
fn x11_dispatch_mit_shm_attach_is_namespace_local_metadata() {
    let namespace = NamespaceId::from_raw(45);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let attach = decode_x11_core_request(
        context(namespace, 528, XByteOrder::LittleEndian),
        &mit_shm_attach_request(XByteOrder::LittleEndian, 0x440010, 88, false),
    )
    .unwrap();

    let result = dispatch_x11_wire_request(
        dispatch_context(
            namespace,
            1,
            XByteOrder::LittleEndian,
            X_MIT_SHM_MAJOR_OPCODE,
        ),
        attach,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );

    assert!(result.outputs.is_empty());
    assert_eq!(runtime.shm_segment_count(), 1);
    assert!(
        runtime
            .validate_shm_segment_access(namespace, XResourceId::new(0x440010, 1))
            .is_ok()
    );
    assert!(
        runtime
            .validate_shm_segment_access(NamespaceId::from_raw(46), XResourceId::new(0x440010, 1))
            .is_err()
    );

    let null_detach = decode_x11_core_request(
        context(namespace, 529, XByteOrder::LittleEndian),
        &mit_shm_detach_request(XByteOrder::LittleEndian, 0),
    )
    .unwrap();
    let null_detach = dispatch_x11_wire_request(
        dispatch_context(
            namespace,
            2,
            XByteOrder::LittleEndian,
            X_MIT_SHM_MAJOR_OPCODE,
        ),
        null_detach,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    assert!(null_detach.outputs.is_empty());
    assert_eq!(runtime.shm_segment_count(), 1);
}

#[test]
fn x11_dispatch_mit_shm_put_image_emits_bounded_surface_transaction() {
    let namespace = NamespaceId::from_raw(45);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();

    let missing = decode_x11_core_request(
        context(namespace, 529, XByteOrder::LittleEndian),
        &mit_shm_put_image_request(XByteOrder::LittleEndian, 0x220701, 0x220702, 0x440020, 0),
    )
    .unwrap();
    let missing = dispatch_x11_wire_request(
        dispatch_context(
            namespace,
            1,
            XByteOrder::LittleEndian,
            X_MIT_SHM_MAJOR_OPCODE,
        ),
        missing,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = missing.encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(
        missing.response.unwrap().outcome,
        XAuthorityResponseOutcome::Accepted
    );
    assert_eq!(encoded[0][0], 0);
    assert_eq!(encoded[0][1], XErrorCode::BadAccess.wire_code());
    assert_eq!(
        read_u32(XByteOrder::LittleEndian, &encoded[0][4..8]),
        0x440020
    );

    runtime
        .attach_shm_segment(namespace, XResourceId::new(0x440020, 1), 88, false, 1)
        .unwrap();
    let create = decode_x11_core_request(
        context(namespace, 530, XByteOrder::LittleEndian),
        &create_window_request(XByteOrder::LittleEndian, 0x220701, 0, 0, 300, 200),
    )
    .unwrap();
    dispatch_x11_wire_request(
        dispatch_context(namespace, 2, XByteOrder::LittleEndian, 1),
        create,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let attached = decode_x11_core_request(
        context(namespace, 531, XByteOrder::LittleEndian),
        &mit_shm_put_image_request(XByteOrder::LittleEndian, 0x220701, 0x220702, 0x440020, 0),
    )
    .unwrap();
    let attached = dispatch_x11_wire_request(
        dispatch_context(
            namespace,
            3,
            XByteOrder::LittleEndian,
            X_MIT_SHM_MAJOR_OPCODE,
        ),
        attached,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );

    assert!(attached.outputs.is_empty());
    let response = attached.response.unwrap();
    assert_eq!(response.outcome, XAuthorityResponseOutcome::Accepted);
    assert_eq!(response.transactions.len(), 1);
    assert_eq!(
        response.transactions[0].surface,
        SurfaceId::new(0x220701, 1)
    );
    assert!(matches!(
        response.transactions[0].target_buffer,
        BufferSource::CpuBuffer { .. }
    ));
    assert_eq!(
        response.transactions[0].damage,
        Region::single(Rect {
            x: 3,
            y: 5,
            width: 32,
            height: 24,
        })
    );
}

#[test]
fn x11_image_text_updates_bounded_xrgb_cpu_pixels() {
    let namespace = NamespaceId::from_raw(45);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let create = decode_x11_core_request(
        context(namespace, 532, XByteOrder::LittleEndian),
        &create_window_request(XByteOrder::LittleEndian, 0x220801, 0, 0, 160, 40),
    )
    .unwrap();
    dispatch_x11_wire_request(
        dispatch_context(namespace, 1, XByteOrder::LittleEndian, 1),
        create,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let gc = decode_x11_core_request(
        context(namespace, 533, XByteOrder::LittleEndian),
        &create_gc_request(XByteOrder::LittleEndian, 0x220802, 0x220801),
    )
    .unwrap();
    dispatch_x11_wire_request(
        dispatch_context(namespace, 2, XByteOrder::LittleEndian, 55),
        gc,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );

    let text = decode_x11_core_request(
        context(namespace, 534, XByteOrder::LittleEndian),
        &image_text8_request(
            XByteOrder::LittleEndian,
            0x220801,
            0x220802,
            4,
            16,
            b"Sophia",
        ),
    )
    .unwrap();
    let result = dispatch_x11_wire_request(
        dispatch_context(namespace, 3, XByteOrder::LittleEndian, 76),
        text,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );

    let response = result.response.unwrap();
    assert_eq!(response.outcome, XAuthorityResponseOutcome::Accepted);
    assert_eq!(response.transactions.len(), 1);
    let XAuthorityCpuBufferUpdate::Replace(snapshot) = runtime.take_cpu_buffer_update().unwrap()
    else {
        panic!("first CPU buffer update must replace the buffer");
    };
    assert_eq!(snapshot.drawable, XResourceId::new(0x220801, 1));
    assert_eq!(
        snapshot.size,
        Size {
            width: 160,
            height: 40
        }
    );
    assert_eq!(snapshot.stride, 640);
    assert_eq!(snapshot.format, X_AUTHORITY_CPU_BUFFER_FORMAT_XRGB8888);
    assert_eq!(snapshot.generation, 1);
    assert!(
        snapshot
            .bytes
            .chunks_exact(4)
            .any(|pixel| pixel != [0, 0, 0, 0])
    );
    assert_eq!(
        response.transactions[0].target_buffer,
        BufferSource::CpuBuffer {
            handle: snapshot.handle
        }
    );
}

#[test]
fn x11_dispatch_reports_empty_extension_list() {
    let namespace = NamespaceId::from_raw(45);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let request = decode_x11_core_request(
        context(namespace, 523, XByteOrder::LittleEndian),
        &[99, 0, 1, 0],
    )
    .unwrap();

    let result = dispatch_x11_wire_request(
        dispatch_context(namespace, 1, XByteOrder::LittleEndian, 99),
        request,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = result.encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(encoded[0][0], 1);
    assert_eq!(encoded[0][1], 0);
    assert_eq!(read_u32(XByteOrder::LittleEndian, &encoded[0][4..8]), 0);
}

#[test]
fn x11_dispatch_query_best_size_echoes_requested_dimensions() {
    let namespace = NamespaceId::from_raw(45);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let mut bytes = vec![97, 0];
    push_u16(&mut bytes, XByteOrder::LittleEndian, 3);
    push_u32(&mut bytes, XByteOrder::LittleEndian, X_SETUP_DEFAULT_ROOT);
    push_u16(&mut bytes, XByteOrder::LittleEndian, 64);
    push_u16(&mut bytes, XByteOrder::LittleEndian, 32);
    let request =
        decode_x11_core_request(context(namespace, 524, XByteOrder::LittleEndian), &bytes).unwrap();

    let result = dispatch_x11_wire_request(
        dispatch_context(namespace, 1, XByteOrder::LittleEndian, 97),
        request,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = result.encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(encoded[0][0], 1);
    assert_eq!(read_u16(XByteOrder::LittleEndian, &encoded[0][8..10]), 64);
    assert_eq!(read_u16(XByteOrder::LittleEndian, &encoded[0][10..12]), 32);
}

#[test]
fn x11_dispatch_get_geometry_reports_root_dimensions() {
    let namespace = NamespaceId::from_raw(45);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let request = decode_x11_core_request(
        context(namespace, 525, XByteOrder::LittleEndian),
        &resource_request(XByteOrder::LittleEndian, 14, X_SETUP_DEFAULT_ROOT),
    )
    .unwrap();

    let result = dispatch_x11_wire_request(
        dispatch_context(namespace, 1, XByteOrder::LittleEndian, 14),
        request,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = result.encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(encoded[0][0], 1);
    assert_eq!(encoded[0][1], 24);
    assert_eq!(
        read_u32(XByteOrder::LittleEndian, &encoded[0][8..12]),
        X_SETUP_DEFAULT_ROOT
    );
    assert_eq!(read_u16(XByteOrder::LittleEndian, &encoded[0][12..14]), 0);
    assert_eq!(read_u16(XByteOrder::LittleEndian, &encoded[0][14..16]), 0);
    assert_eq!(
        read_u16(XByteOrder::LittleEndian, &encoded[0][16..18]),
        X_SETUP_ROOT_WIDTH
    );
    assert_eq!(
        read_u16(XByteOrder::LittleEndian, &encoded[0][18..20]),
        X_SETUP_ROOT_HEIGHT
    );
}

#[test]
fn x11_dispatch_get_window_attributes_reports_root_visual_state() {
    let namespace = NamespaceId::from_raw(45);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let request = decode_x11_core_request(
        context(namespace, 527, XByteOrder::LittleEndian),
        &resource_request(XByteOrder::LittleEndian, 3, X_SETUP_DEFAULT_ROOT),
    )
    .unwrap();

    let result = dispatch_x11_wire_request(
        dispatch_context(namespace, 1, XByteOrder::LittleEndian, 3),
        request,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = result.encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(encoded[0][0], 1);
    assert_eq!(read_u32(XByteOrder::LittleEndian, &encoded[0][4..8]), 3);
    assert_eq!(
        read_u32(XByteOrder::LittleEndian, &encoded[0][8..12]),
        X_SETUP_DEFAULT_VISUAL
    );
    assert_eq!(read_u16(XByteOrder::LittleEndian, &encoded[0][12..14]), 1);
    assert_eq!(encoded[0][26], 2);
    assert_eq!(
        read_u32(XByteOrder::LittleEndian, &encoded[0][28..32]),
        X_SETUP_DEFAULT_COLORMAP
    );
}

#[test]
fn x11_dispatch_query_tree_reports_root_without_children() {
    let namespace = NamespaceId::from_raw(45);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let request = decode_x11_core_request(
        context(namespace, 528, XByteOrder::LittleEndian),
        &resource_request(XByteOrder::LittleEndian, 15, X_SETUP_DEFAULT_ROOT),
    )
    .unwrap();

    let result = dispatch_x11_wire_request(
        dispatch_context(namespace, 1, XByteOrder::LittleEndian, 15),
        request,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = result.encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(encoded[0][0], 1);
    assert_eq!(read_u32(XByteOrder::LittleEndian, &encoded[0][4..8]), 0);
    assert_eq!(
        read_u32(XByteOrder::LittleEndian, &encoded[0][8..12]),
        X_SETUP_DEFAULT_ROOT
    );
    assert_eq!(read_u32(XByteOrder::LittleEndian, &encoded[0][12..16]), 0);
    assert_eq!(read_u16(XByteOrder::LittleEndian, &encoded[0][16..18]), 0);
}

#[test]
fn x11_dispatch_randr_reports_root_screen_size_and_populated_resources() {
    let namespace = NamespaceId::from_raw(45);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let range = decode_x11_core_request(
        context(namespace, 540, XByteOrder::LittleEndian),
        &randr_window_request(
            XByteOrder::LittleEndian,
            X_RANDR_GET_SCREEN_SIZE_RANGE_MINOR_OPCODE,
            X_SETUP_DEFAULT_ROOT,
        ),
    )
    .unwrap();
    assert_eq!(
        range,
        XWireRequest::RandrGetScreenSizeRange {
            window: XResourceId::new(u64::from(X_SETUP_DEFAULT_ROOT), 1),
        }
    );

    let range = dispatch_x11_wire_request(
        dispatch_context(namespace, 1, XByteOrder::LittleEndian, X_RANDR_MAJOR_OPCODE),
        range,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = range.encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(encoded[0][0], 1);
    assert_eq!(
        read_u16(XByteOrder::LittleEndian, &encoded[0][8..10]),
        X_SETUP_ROOT_WIDTH
    );
    assert_eq!(
        read_u16(XByteOrder::LittleEndian, &encoded[0][10..12]),
        X_SETUP_ROOT_HEIGHT
    );
    assert_eq!(
        read_u16(XByteOrder::LittleEndian, &encoded[0][12..14]),
        X_SETUP_ROOT_WIDTH
    );
    assert_eq!(
        read_u16(XByteOrder::LittleEndian, &encoded[0][14..16]),
        X_SETUP_ROOT_HEIGHT
    );

    let resources = decode_x11_core_request(
        context(namespace, 541, XByteOrder::LittleEndian),
        &randr_window_request(
            XByteOrder::LittleEndian,
            X_RANDR_GET_SCREEN_RESOURCES_MINOR_OPCODE,
            X_SETUP_DEFAULT_ROOT,
        ),
    )
    .unwrap();
    assert_eq!(
        resources,
        XWireRequest::RandrGetScreenResources {
            window: XResourceId::new(u64::from(X_SETUP_DEFAULT_ROOT), 1),
            current: false,
        }
    );
    let resources = dispatch_x11_wire_request(
        dispatch_context(namespace, 2, XByteOrder::LittleEndian, X_RANDR_MAJOR_OPCODE),
        resources,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = resources.encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(encoded[0][0], 1);
    assert!(read_u32(XByteOrder::LittleEndian, &encoded[0][4..8]) > 0);
    assert_eq!(read_u16(XByteOrder::LittleEndian, &encoded[0][16..18]), 1);
    assert_eq!(read_u16(XByteOrder::LittleEndian, &encoded[0][18..20]), 1);
    assert_eq!(read_u16(XByteOrder::LittleEndian, &encoded[0][20..22]), 1);
    assert!(read_u16(XByteOrder::LittleEndian, &encoded[0][22..24]) > 0);
}

#[test]
fn x11_dispatch_list_properties_reports_window_property_atoms() {
    let namespace = NamespaceId::from_raw(45);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let utf8 = atoms
        .intern(X_ATOM_NAME_UTF8_STRING, false)
        .unwrap()
        .unwrap();
    let net_wm_name = atoms
        .intern(X_ATOM_NAME_NET_WM_NAME, false)
        .unwrap()
        .unwrap();
    properties
        .apply_change(
            namespace,
            XPropertyChange {
                mode: XPropertyMode::Replace,
                window: XResourceId::new(u64::from(X_SETUP_DEFAULT_ROOT), 1),
                property: net_wm_name,
                property_type: utf8,
                format: 8,
                bytes: b"Sophia root".to_vec(),
            },
        )
        .unwrap();
    let request = decode_x11_core_request(
        context(namespace, 529, XByteOrder::LittleEndian),
        &resource_request(XByteOrder::LittleEndian, 21, X_SETUP_DEFAULT_ROOT),
    )
    .unwrap();

    let result = dispatch_x11_wire_request(
        dispatch_context(namespace, 1, XByteOrder::LittleEndian, 21),
        request,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = result.encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(encoded[0][0], 1);
    assert_eq!(read_u32(XByteOrder::LittleEndian, &encoded[0][4..8]), 1);
    assert_eq!(read_u16(XByteOrder::LittleEndian, &encoded[0][8..10]), 1);
    assert_eq!(
        read_u32(XByteOrder::LittleEndian, &encoded[0][32..36]),
        net_wm_name
    );
}

#[test]
fn x11_dispatch_translate_coordinates_echoes_root_coordinates() {
    let namespace = NamespaceId::from_raw(45);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let request = decode_x11_core_request(
        context(namespace, 526, XByteOrder::LittleEndian),
        &translate_coordinates_request(
            XByteOrder::LittleEndian,
            X_SETUP_DEFAULT_ROOT,
            X_SETUP_DEFAULT_ROOT,
            12,
            34,
        ),
    )
    .unwrap();

    let result = dispatch_x11_wire_request(
        dispatch_context(namespace, 1, XByteOrder::LittleEndian, 40),
        request,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = result.encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(encoded[0][0], 1);
    assert_eq!(encoded[0][1], 1);
    assert_eq!(read_u32(XByteOrder::LittleEndian, &encoded[0][8..12]), 0);
    assert_eq!(read_i16(XByteOrder::LittleEndian, &encoded[0][12..14]), 12);
    assert_eq!(read_i16(XByteOrder::LittleEndian, &encoded[0][14..16]), 34);
}

#[test]
fn x11_dispatch_query_colors_returns_bounded_color_records() {
    let namespace = NamespaceId::from_raw(45);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let request = decode_x11_core_request(
        context(namespace, 525, XByteOrder::LittleEndian),
        &query_colors_request(XByteOrder::LittleEndian, X_SETUP_DEFAULT_ROOT, &[0, 1]),
    )
    .unwrap();

    assert_eq!(
        request,
        XWireRequest::QueryColors {
            colormap: XResourceId::new(u64::from(X_SETUP_DEFAULT_ROOT), 1),
            pixels: vec![0, 1],
        }
    );

    let result = dispatch_x11_wire_request(
        dispatch_context(namespace, 1, XByteOrder::LittleEndian, 91),
        request,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = result.encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(encoded[0][0], 1);
    assert_eq!(read_u32(XByteOrder::LittleEndian, &encoded[0][4..8]), 4);
    assert_eq!(read_u16(XByteOrder::LittleEndian, &encoded[0][8..10]), 2);
    assert_eq!(read_u16(XByteOrder::LittleEndian, &encoded[0][32..34]), 0);
    assert_eq!(
        read_u16(XByteOrder::LittleEndian, &encoded[0][40..42]),
        u16::MAX
    );
    assert_eq!(
        read_u16(XByteOrder::LittleEndian, &encoded[0][42..44]),
        u16::MAX
    );
    assert_eq!(
        read_u16(XByteOrder::LittleEndian, &encoded[0][44..46]),
        u16::MAX
    );
}

#[test]
fn x11_dispatch_create_colormap_accepts_root_visual() {
    let namespace = NamespaceId::from_raw(45);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let request = decode_x11_core_request(
        context(namespace, 526, XByteOrder::LittleEndian),
        &create_colormap_request(
            XByteOrder::LittleEndian,
            0x200001,
            X_SETUP_DEFAULT_ROOT,
            X_SETUP_DEFAULT_VISUAL,
        ),
    )
    .unwrap();

    assert_eq!(
        request,
        XWireRequest::CreateColormap {
            colormap: XResourceId::new(0x200001, 1),
            window: XResourceId::new(u64::from(X_SETUP_DEFAULT_ROOT), 1),
            visual: X_SETUP_DEFAULT_VISUAL,
        }
    );

    let result = dispatch_x11_wire_request(
        dispatch_context(namespace, 1, XByteOrder::LittleEndian, 78),
        request,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    assert!(result.outputs.is_empty());
}

#[test]
fn x11_dispatch_alloc_named_color_returns_reduced_black_white_pixels() {
    let namespace = NamespaceId::from_raw(45);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let request = decode_x11_core_request(
        context(namespace, 542, XByteOrder::LittleEndian),
        &alloc_named_color_request(XByteOrder::LittleEndian, X_SETUP_DEFAULT_COLORMAP, "black"),
    )
    .unwrap();

    assert_eq!(
        request,
        XWireRequest::AllocNamedColor {
            colormap: XResourceId::new(u64::from(X_SETUP_DEFAULT_COLORMAP), 1),
            name: "black".to_owned(),
        }
    );

    let result = dispatch_x11_wire_request(
        dispatch_context(namespace, 1, XByteOrder::LittleEndian, 85),
        request,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = result.encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(encoded[0][0], 1);
    assert_eq!(read_u32(XByteOrder::LittleEndian, &encoded[0][8..12]), 0);
    assert_eq!(read_u16(XByteOrder::LittleEndian, &encoded[0][12..14]), 0);
    assert_eq!(read_u16(XByteOrder::LittleEndian, &encoded[0][18..20]), 0);

    let white = decode_x11_core_request(
        context(namespace, 543, XByteOrder::LittleEndian),
        &alloc_named_color_request(XByteOrder::LittleEndian, X_SETUP_DEFAULT_COLORMAP, "white"),
    )
    .unwrap();
    let white = dispatch_x11_wire_request(
        dispatch_context(namespace, 2, XByteOrder::LittleEndian, 85),
        white,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = white.encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(read_u32(XByteOrder::LittleEndian, &encoded[0][8..12]), 1);
    assert_eq!(
        read_u16(XByteOrder::LittleEndian, &encoded[0][12..14]),
        u16::MAX
    );
    assert_eq!(
        read_u16(XByteOrder::LittleEndian, &encoded[0][18..20]),
        u16::MAX
    );
}

#[test]
fn x11_dispatch_alloc_color_echoes_reduced_rgb_pixel() {
    let namespace = NamespaceId::from_raw(45);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let request = decode_x11_core_request(
        context(namespace, 544, XByteOrder::LittleEndian),
        &alloc_color_request(
            XByteOrder::LittleEndian,
            X_SETUP_DEFAULT_COLORMAP,
            0xff00,
            0,
            0,
        ),
    )
    .unwrap();

    assert_eq!(
        request,
        XWireRequest::AllocColor {
            colormap: XResourceId::new(u64::from(X_SETUP_DEFAULT_COLORMAP), 1),
            red: 0xff00,
            green: 0,
            blue: 0,
        }
    );

    let result = dispatch_x11_wire_request(
        dispatch_context(namespace, 3, XByteOrder::LittleEndian, 84),
        request,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = result.encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(encoded[0][0], 1);
    assert_eq!(
        read_u16(XByteOrder::LittleEndian, &encoded[0][8..10]),
        0xff00
    );
    assert_eq!(read_u16(XByteOrder::LittleEndian, &encoded[0][10..12]), 0);
    assert_eq!(read_u16(XByteOrder::LittleEndian, &encoded[0][12..14]), 0);
    assert_eq!(
        read_u32(XByteOrder::LittleEndian, &encoded[0][16..20]),
        0xff0000
    );
}

#[test]
fn x11_dispatch_reads_bounded_property_slices() {
    let namespace = NamespaceId::from_raw(45);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let utf8 = atoms
        .intern(X_ATOM_NAME_UTF8_STRING, false)
        .unwrap()
        .unwrap();
    let net_wm_name = atoms
        .intern(X_ATOM_NAME_NET_WM_NAME, false)
        .unwrap()
        .unwrap();
    let window = 0x220008;

    let create = decode_x11_core_request(
        context(namespace, 513, XByteOrder::LittleEndian),
        &create_window_request(XByteOrder::LittleEndian, window, 0, 0, 300, 200),
    )
    .unwrap();
    dispatch_x11_wire_request(
        dispatch_context(namespace, 1, XByteOrder::LittleEndian, 1),
        create,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );

    let title = b"Secret Terminal Title";
    let change = decode_x11_core_request(
        context(namespace, 514, XByteOrder::LittleEndian),
        &change_property_request(
            XByteOrder::LittleEndian,
            XPropertyMode::Replace,
            window,
            net_wm_name,
            utf8,
            8,
            title,
        ),
    )
    .unwrap();
    dispatch_x11_wire_request(
        dispatch_context(namespace, 2, XByteOrder::LittleEndian, 18),
        change,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );

    let read = decode_x11_core_request(
        context(namespace, 515, XByteOrder::LittleEndian),
        &get_property_request(
            XByteOrder::LittleEndian,
            false,
            window,
            net_wm_name,
            X_PROPERTY_ANY_TYPE,
            1,
            2,
        ),
    )
    .unwrap();
    let read = dispatch_x11_wire_request(
        dispatch_context(namespace, 3, XByteOrder::LittleEndian, 20),
        read,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = read.encoded_outputs(XByteOrder::LittleEndian);

    assert_eq!(encoded[0][0], 1);
    assert_eq!(encoded[0][1], 8);
    assert_eq!(read_u32(XByteOrder::LittleEndian, &encoded[0][8..12]), utf8);
    assert_eq!(
        read_u32(XByteOrder::LittleEndian, &encoded[0][12..16]),
        u32::try_from(title.len() - 12).unwrap()
    );
    assert_eq!(read_u32(XByteOrder::LittleEndian, &encoded[0][16..20]), 8);
    assert_eq!(&encoded[0][32..40], &title[4..12]);
}

#[test]
fn x11_dispatch_get_selection_owner_reports_no_owner() {
    let namespace = NamespaceId::from_raw(45);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let request = decode_x11_core_request(
        context(namespace, 544, XByteOrder::LittleEndian),
        &resource_request(XByteOrder::LittleEndian, 23, 7),
    )
    .unwrap();

    let result = dispatch_x11_wire_request(
        dispatch_context(namespace, 1, XByteOrder::LittleEndian, 23),
        request,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = result.encoded_outputs(XByteOrder::LittleEndian);

    assert_eq!(encoded.len(), 1);
    assert_eq!(encoded[0][0], 1);
    assert_eq!(read_u32(XByteOrder::LittleEndian, &encoded[0][8..12]), 0);
}

#[test]
fn x11_dispatch_accepts_root_button_grab_lifecycle() {
    let namespace = NamespaceId::from_raw(45);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();

    let grab = decode_x11_core_request(
        context(namespace, 545, XByteOrder::LittleEndian),
        &grab_button_request(
            XByteOrder::LittleEndian,
            X_SETUP_DEFAULT_ROOT,
            0x001c,
            1,
            0x0040,
        ),
    )
    .unwrap();
    let grab = dispatch_x11_wire_request(
        dispatch_context(namespace, 1, XByteOrder::LittleEndian, 28),
        grab,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    assert!(grab.outputs.is_empty());

    let ungrab = decode_x11_core_request(
        context(namespace, 546, XByteOrder::LittleEndian),
        &ungrab_button_request(XByteOrder::LittleEndian, X_SETUP_DEFAULT_ROOT, 1, 0x0040),
    )
    .unwrap();
    let ungrab = dispatch_x11_wire_request(
        dispatch_context(namespace, 2, XByteOrder::LittleEndian, 29),
        ungrab,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    assert!(ungrab.outputs.is_empty());
}

#[test]
fn x11_dispatch_allows_empty_root_property_reads() {
    let namespace = NamespaceId::from_raw(45);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let read = decode_x11_core_request(
        context(namespace, 525, XByteOrder::LittleEndian),
        &get_property_request(
            XByteOrder::LittleEndian,
            false,
            X_SETUP_DEFAULT_ROOT,
            X_ATOM_RESOURCE_MANAGER,
            X_PROPERTY_ANY_TYPE,
            0,
            64,
        ),
    )
    .unwrap();

    let result = dispatch_x11_wire_request(
        dispatch_context(namespace, 1, XByteOrder::LittleEndian, 20),
        read,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = result.encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(encoded[0][0], 1);
    assert_eq!(encoded[0][1], 0);
    assert_eq!(read_u32(XByteOrder::LittleEndian, &encoded[0][8..12]), 0);
}

#[test]
fn x11_dispatch_get_property_fails_closed_for_bad_window_atom_and_offset() {
    let namespace = NamespaceId::from_raw(45);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let utf8 = atoms
        .intern(X_ATOM_NAME_UTF8_STRING, false)
        .unwrap()
        .unwrap();

    let bad_window = decode_x11_core_request(
        context(namespace, 516, XByteOrder::LittleEndian),
        &get_property_request(
            XByteOrder::LittleEndian,
            false,
            0x220009,
            X_ATOM_WM_NAME,
            X_PROPERTY_ANY_TYPE,
            0,
            1,
        ),
    )
    .unwrap();
    let bad_window = dispatch_x11_wire_request(
        dispatch_context(namespace, 1, XByteOrder::LittleEndian, 20),
        bad_window,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    assert_eq!(
        bad_window.encoded_outputs(XByteOrder::LittleEndian)[0][1],
        3
    );

    let create = decode_x11_core_request(
        context(namespace, 517, XByteOrder::LittleEndian),
        &create_window_request(XByteOrder::LittleEndian, 0x220009, 0, 0, 300, 200),
    )
    .unwrap();
    dispatch_x11_wire_request(
        dispatch_context(namespace, 2, XByteOrder::LittleEndian, 1),
        create,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );

    let bad_atom = decode_x11_core_request(
        context(namespace, 518, XByteOrder::LittleEndian),
        &get_property_request(
            XByteOrder::LittleEndian,
            false,
            0x220009,
            0x00ff_ffff,
            X_PROPERTY_ANY_TYPE,
            0,
            1,
        ),
    )
    .unwrap();
    let bad_atom = dispatch_x11_wire_request(
        dispatch_context(namespace, 3, XByteOrder::LittleEndian, 20),
        bad_atom,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    assert_eq!(
        bad_atom.encoded_outputs(XByteOrder::LittleEndian)[0][1],
        XErrorCode::BadAtom.wire_code()
    );

    let change = decode_x11_core_request(
        context(namespace, 519, XByteOrder::LittleEndian),
        &change_property_request(
            XByteOrder::LittleEndian,
            XPropertyMode::Replace,
            0x220009,
            X_ATOM_WM_NAME,
            utf8,
            8,
            b"short",
        ),
    )
    .unwrap();
    dispatch_x11_wire_request(
        dispatch_context(namespace, 4, XByteOrder::LittleEndian, 18),
        change,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );

    let bad_offset = decode_x11_core_request(
        context(namespace, 520, XByteOrder::LittleEndian),
        &get_property_request(
            XByteOrder::LittleEndian,
            false,
            0x220009,
            X_ATOM_WM_NAME,
            X_PROPERTY_ANY_TYPE,
            2,
            1,
        ),
    )
    .unwrap();
    let bad_offset = dispatch_x11_wire_request(
        dispatch_context(namespace, 5, XByteOrder::LittleEndian, 20),
        bad_offset,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    assert_eq!(
        bad_offset.encoded_outputs(XByteOrder::LittleEndian)[0][1],
        XErrorCode::BadValue.wire_code()
    );
}

#[test]
fn x11_property_records_emit_metadata_candidates_without_raw_payloads() {
    let namespace = NamespaceId::from_raw(45);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let utf8 = atoms
        .intern(X_ATOM_NAME_UTF8_STRING, false)
        .unwrap()
        .unwrap();
    let net_wm_name = atoms
        .intern(X_ATOM_NAME_NET_WM_NAME, false)
        .unwrap()
        .unwrap();
    let window = 0x220006;
    let create = decode_x11_core_request(
        context(namespace, 511, XByteOrder::LittleEndian),
        &create_window_request(XByteOrder::LittleEndian, window, 0, 0, 320, 200),
    )
    .unwrap();
    dispatch_x11_wire_request(
        dispatch_context(namespace, 4, XByteOrder::LittleEndian, 1),
        create,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let decoded = decode_x11_core_request(
        context(namespace, 512, XByteOrder::LittleEndian),
        &change_property_request(
            XByteOrder::LittleEndian,
            XPropertyMode::Replace,
            window,
            net_wm_name,
            utf8,
            8,
            b"Secret Terminal Title",
        ),
    )
    .unwrap();

    let result = dispatch_x11_wire_request(
        dispatch_context(namespace, 5, XByteOrder::LittleEndian, 18),
        decoded,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );

    assert_eq!(result.outputs.len(), 1);
    assert_eq!(result.metadata_candidates.len(), 1);
    let candidate = &result.metadata_candidates[0];
    assert_eq!(candidate.namespace, namespace);
    assert_eq!(candidate.window, XResourceId::new(u64::from(window), 1));
    assert_eq!(candidate.property_name, X_ATOM_NAME_NET_WM_NAME);
    assert_eq!(
        candidate.property_type_name.as_deref(),
        Some(X_ATOM_NAME_UTF8_STRING)
    );
    assert_eq!(candidate.byte_len, b"Secret Terminal Title".len());
}

#[test]
fn x11_core_decoder_rejects_bad_lengths_and_unknown_opcodes() {
    assert_eq!(
        decode_x11_core_request(
            context(NamespaceId::from_raw(45), 506, XByteOrder::LittleEndian),
            &[1, 0, 1]
        ),
        Err(XWireParseError::Truncated {
            needed: 4,
            actual: 3,
        })
    );

    let mut unknown = vec![127, 0];
    push_u16(&mut unknown, XByteOrder::LittleEndian, 1);
    assert_eq!(
        decode_x11_core_request(
            context(NamespaceId::from_raw(45), 507, XByteOrder::LittleEndian),
            &unknown
        ),
        Err(XWireParseError::UnknownOpcode(127))
    );

    let mut unsupported_shm_minor = vec![X_MIT_SHM_MAJOR_OPCODE, 99];
    push_u16(&mut unsupported_shm_minor, XByteOrder::LittleEndian, 1);
    assert_eq!(
        decode_x11_core_request(
            context(NamespaceId::from_raw(45), 507, XByteOrder::LittleEndian),
            &unsupported_shm_minor
        ),
        Err(XWireParseError::UnknownOpcode(X_MIT_SHM_MAJOR_OPCODE))
    );

    let mut oversized_map = vec![8, 0];
    push_u16(&mut oversized_map, XByteOrder::LittleEndian, 3);
    push_u32(&mut oversized_map, XByteOrder::LittleEndian, 0x220005);
    push_u32(&mut oversized_map, XByteOrder::LittleEndian, 0);
    assert_eq!(
        decode_x11_core_request(
            context(NamespaceId::from_raw(45), 508, XByteOrder::LittleEndian),
            &oversized_map
        ),
        Err(XWireParseError::InvalidLength {
            opcode: 8,
            expected_at_least: 8,
            actual: 12,
        })
    );
}

#[test]
fn x11_client_event_encoders_emit_32_byte_records() {
    let map = encode_x_client_output(
        XByteOrder::LittleEndian,
        XClientOutput::Event(XClientEvent::MapNotify {
            sequence: 9,
            event: XResourceId::new(0x220001, 1),
            window: XResourceId::new(0x220001, 1),
            override_redirect: false,
        }),
    );
    assert_eq!(map.len(), 32);
    assert_eq!(map[0], 19);
    assert_eq!(read_u16(XByteOrder::LittleEndian, &map[2..4]), 9);
    assert_eq!(read_u32(XByteOrder::LittleEndian, &map[4..8]), 0x220001);
    assert_eq!(read_u32(XByteOrder::LittleEndian, &map[8..12]), 0x220001);

    let configure = encode_x_client_output(
        XByteOrder::BigEndian,
        XClientOutput::Event(XClientEvent::ConfigureNotify {
            sequence: 10,
            event: XResourceId::new(0x220002, 1),
            window: XResourceId::new(0x220002, 1),
            above_sibling: None,
            x: 12,
            y: 13,
            width: 640,
            height: 480,
            border_width: 0,
            override_redirect: false,
        }),
    );
    assert_eq!(configure[0], 22);
    assert_eq!(read_u16(XByteOrder::BigEndian, &configure[2..4]), 10);
    assert_eq!(read_u32(XByteOrder::BigEndian, &configure[8..12]), 0x220002);
    assert_eq!(read_u16(XByteOrder::BigEndian, &configure[20..22]), 640);
    assert_eq!(read_u16(XByteOrder::BigEndian, &configure[22..24]), 480);

    let key = encode_x_client_output(
        XByteOrder::LittleEndian,
        XClientOutput::Event(XClientEvent::Key {
            sequence: 11,
            pressed: true,
            keycode: 38,
            time: 123,
            root: XResourceId::new(u64::from(X_SETUP_DEFAULT_ROOT), 1),
            event: XResourceId::new(0x220003, 1),
            state: 1,
        }),
    );
    assert_eq!(key.len(), 32);
    assert_eq!(key[0], 2);
    assert_eq!(key[1], 38);
    assert_eq!(read_u16(XByteOrder::LittleEndian, &key[2..4]), 11);
    assert_eq!(read_u32(XByteOrder::LittleEndian, &key[4..8]), 123);
    assert_eq!(read_u32(XByteOrder::LittleEndian, &key[12..16]), 0x220003);
    assert_eq!(read_u16(XByteOrder::LittleEndian, &key[28..30]), 1);
    assert_eq!(key[30], 1);

    let focus = encode_x_client_output(
        XByteOrder::BigEndian,
        XClientOutput::Event(XClientEvent::Focus {
            sequence: 12,
            focused: true,
            detail: 3,
            event: XResourceId::new(0x220003, 1),
            mode: 0,
        }),
    );
    assert_eq!(focus.len(), 32);
    assert_eq!(focus[0], 9);
    assert_eq!(focus[1], 3);
    assert_eq!(read_u16(XByteOrder::BigEndian, &focus[2..4]), 12);
    assert_eq!(read_u32(XByteOrder::BigEndian, &focus[4..8]), 0x220003);
    assert_eq!(focus[8], 0);

    let motion = encode_x_client_output(
        XByteOrder::LittleEndian,
        XClientOutput::Event(XClientEvent::PointerMotion {
            sequence: 12,
            time: 124,
            root: XResourceId::new(u64::from(X_SETUP_DEFAULT_ROOT), 1),
            event: XResourceId::new(0x220003, 1),
            root_x: 50,
            root_y: 60,
            event_x: 10,
            event_y: 20,
            state: 1 << 8,
        }),
    );
    assert_eq!(motion[0], 6);
    assert_eq!(motion[1], 0);
    assert_eq!(read_u16(XByteOrder::LittleEndian, &motion[2..4]), 12);
    assert_eq!(read_u16(XByteOrder::LittleEndian, &motion[24..26]), 10);
    assert_eq!(read_u16(XByteOrder::LittleEndian, &motion[28..30]), 1 << 8);

    let button = encode_x_client_output(
        XByteOrder::LittleEndian,
        XClientOutput::Event(XClientEvent::PointerButton {
            sequence: 13,
            pressed: true,
            button: 1,
            time: 125,
            root: XResourceId::new(u64::from(X_SETUP_DEFAULT_ROOT), 1),
            event: XResourceId::new(0x220003, 1),
            root_x: 50,
            root_y: 60,
            event_x: 10,
            event_y: 20,
            state: 0,
        }),
    );
    assert_eq!(button[0], 4);
    assert_eq!(button[1], 1);
    assert_eq!(read_u16(XByteOrder::LittleEndian, &button[2..4]), 13);
}

#[test]
fn x11_client_error_encoder_and_parse_mapping_use_core_error_shape() {
    let error = x_error_from_wire_parse(&XWireParseError::UnknownOpcode(99), 11, 99, 7);
    assert_eq!(error.code, XErrorCode::BadRequest);

    let encoded = encode_x_client_output(XByteOrder::LittleEndian, XClientOutput::Error(error));
    assert_eq!(encoded.len(), 32);
    assert_eq!(encoded[0], 0);
    assert_eq!(encoded[1], 1);
    assert_eq!(read_u16(XByteOrder::LittleEndian, &encoded[2..4]), 11);
    assert_eq!(read_u16(XByteOrder::LittleEndian, &encoded[8..10]), 7);
    assert_eq!(encoded[10], 99);

    let bad_length = x_error_from_wire_parse(
        &XWireParseError::InvalidLength {
            opcode: 8,
            expected_at_least: 8,
            actual: 12,
        },
        12,
        8,
        0,
    );
    assert_eq!(bad_length.code, XErrorCode::BadLength);
}

#[test]
fn x11_dispatch_emits_configure_map_property_and_selection_failure_outputs() {
    let namespace = NamespaceId::from_raw(46);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();

    let create = decode_x11_core_request(
        context(namespace, 601, XByteOrder::LittleEndian),
        &create_window_request(XByteOrder::LittleEndian, 0x220101, 10, 20, 640, 480),
    )
    .unwrap();
    let create = dispatch_x11_wire_request(
        dispatch_context(namespace, 1, XByteOrder::LittleEndian, 1),
        create,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    assert_eq!(create.outputs.len(), 1);
    assert_eq!(
        encode_x_client_output(XByteOrder::LittleEndian, create.outputs[0].clone())[0],
        22
    );

    let map = decode_x11_core_request(
        context(namespace, 602, XByteOrder::LittleEndian),
        &resource_request(XByteOrder::LittleEndian, 8, 0x220101),
    )
    .unwrap();
    let map = dispatch_x11_wire_request(
        dispatch_context(namespace, 2, XByteOrder::LittleEndian, 8),
        map,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    assert_eq!(map.outputs.len(), 2);
    assert_eq!(
        encode_x_client_output(XByteOrder::LittleEndian, map.outputs[0].clone())[0],
        19
    );
    assert_eq!(
        encode_x_client_output(XByteOrder::LittleEndian, map.outputs[1].clone())[0],
        12
    );

    let unmap = decode_x11_core_request(
        context(namespace, 603, XByteOrder::LittleEndian),
        &resource_request(XByteOrder::LittleEndian, 10, 0x220101),
    )
    .unwrap();
    let unmap = dispatch_x11_wire_request(
        dispatch_context(namespace, 3, XByteOrder::LittleEndian, 10),
        unmap,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    assert!(unmap.outputs.is_empty());

    let configure = decode_x11_core_request(
        context(namespace, 604, XByteOrder::LittleEndian),
        &configure_window_request(XByteOrder::LittleEndian, 0x220101, 0x000c, &[12, 14]),
    )
    .unwrap();
    let configure = dispatch_x11_wire_request(
        dispatch_context(namespace, 4, XByteOrder::LittleEndian, 12),
        configure,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    assert_eq!(configure.outputs.len(), 1);
    assert_eq!(
        configure.outputs[0],
        XClientOutput::Event(XClientEvent::ConfigureNotify {
            sequence: 4,
            event: XResourceId::new(0x220101, 1),
            window: XResourceId::new(0x220101, 1),
            above_sibling: None,
            x: 10,
            y: 20,
            width: 12,
            height: 14,
            border_width: 0,
            override_redirect: false,
        })
    );
    assert_eq!(
        runtime
            .window_geometry(namespace, XResourceId::new(0x220101, 1))
            .unwrap(),
        Rect {
            x: 10,
            y: 20,
            width: 12,
            height: 14,
        }
    );

    let map_subwindows = decode_x11_core_request(
        context(namespace, 605, XByteOrder::LittleEndian),
        &resource_request(XByteOrder::LittleEndian, 9, X_SETUP_DEFAULT_ROOT),
    )
    .unwrap();
    let map_subwindows = dispatch_x11_wire_request(
        dispatch_context(namespace, 5, XByteOrder::LittleEndian, 9),
        map_subwindows,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    assert_eq!(map_subwindows.outputs.len(), 2);
    assert_eq!(
        encode_x_client_output(XByteOrder::LittleEndian, map_subwindows.outputs[0].clone())[0],
        19
    );
    assert_eq!(
        encode_x_client_output(XByteOrder::LittleEndian, map_subwindows.outputs[1].clone())[0],
        12
    );

    let attributes = decode_x11_core_request(
        context(namespace, 606, XByteOrder::LittleEndian),
        &change_window_attributes_request(XByteOrder::LittleEndian, X_SETUP_DEFAULT_ROOT),
    )
    .unwrap();
    let attributes = dispatch_x11_wire_request(
        dispatch_context(namespace, 6, XByteOrder::LittleEndian, 2),
        attributes,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    assert!(attributes.outputs.is_empty());

    let property = decode_x11_core_request(
        context(namespace, 607, XByteOrder::LittleEndian),
        &change_property_request(
            XByteOrder::LittleEndian,
            XPropertyMode::Replace,
            0x220101,
            7,
            8,
            8,
            b"hello",
        ),
    )
    .unwrap();
    let property = dispatch_x11_wire_request(
        dispatch_context(namespace, 7, XByteOrder::LittleEndian, 18),
        property,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    assert_eq!(property.outputs.len(), 1);
    assert_eq!(
        encode_x_client_output(XByteOrder::LittleEndian, property.outputs[0].clone())[0],
        28
    );

    let selection = decode_x11_core_request(
        context(namespace, 608, XByteOrder::LittleEndian),
        &convert_selection_request(XByteOrder::LittleEndian, 0x220101, 100, 101, 102, 33),
    )
    .unwrap();
    let selection = dispatch_x11_wire_request(
        dispatch_context(namespace, 4, XByteOrder::LittleEndian, 24),
        selection,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    assert_eq!(selection.outputs.len(), 1);
    let encoded = encode_x_client_output(XByteOrder::LittleEndian, selection.outputs[0].clone());
    assert_eq!(encoded[0], 31);
    assert_eq!(
        read_u32(XByteOrder::LittleEndian, &encoded[20..24]),
        X_ATOM_NONE
    );
}

#[test]
fn x11_dispatch_accepts_destroy_window_for_known_namespace_window() {
    let namespace = NamespaceId::from_raw(46);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let create = decode_x11_core_request(
        context(namespace, 601, XByteOrder::LittleEndian),
        &create_window_request(XByteOrder::LittleEndian, 0x220101, 10, 20, 640, 480),
    )
    .unwrap();
    let create = dispatch_x11_wire_request(
        dispatch_context(namespace, 1, XByteOrder::LittleEndian, 1),
        create,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let surface = create
        .response
        .as_ref()
        .expect("CreateWindow should produce an authority response")
        .surfaces
        .first()
        .expect("CreateWindow should create one surface")
        .surface;
    assert_eq!(runtime.window_count(), 1);
    assert_eq!(runtime.resource_count(), 1);

    let destroy = decode_x11_core_request(
        context(namespace, 602, XByteOrder::LittleEndian),
        &resource_request(XByteOrder::LittleEndian, 4, 0x220101),
    )
    .unwrap();
    let destroy = dispatch_x11_wire_request(
        dispatch_context(namespace, 2, XByteOrder::LittleEndian, 4),
        destroy,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );

    assert!(destroy.outputs.is_empty());
    assert_eq!(
        destroy.response.as_ref().unwrap().removed_surfaces,
        vec![surface]
    );
    assert_eq!(runtime.window_count(), 0);
    assert_eq!(runtime.resource_count(), 0);
    assert_eq!(
        XAuthorityObservedTransactionBatch::from_dispatch_result(&destroy),
        Some(XAuthorityObservedTransactionBatch {
            client: None,
            transaction: TransactionId::from_raw(2),
            transactions: Vec::new(),
            removed_surfaces: vec![surface],
            cpu_buffer_updates: Vec::new(),
            dma_buf_registrations: Vec::new(),
            fence_registrations: Vec::new(),
            present_submissions: Vec::new(),
            released_dma_bufs: Vec::new(),
            released_fences: Vec::new(),
            protocol_errors: Vec::new(),
        })
    );
}

#[test]
fn x11_dispatch_poly_fill_rectangle_emits_core_draw_transaction() {
    let namespace = NamespaceId::from_raw(46);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let create = decode_x11_core_request(
        context(namespace, 601, XByteOrder::LittleEndian),
        &create_window_request(XByteOrder::LittleEndian, 0x220101, 10, 20, 640, 480),
    )
    .unwrap();
    dispatch_x11_wire_request(
        dispatch_context(namespace, 1, XByteOrder::LittleEndian, 1),
        create,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let gc = decode_x11_core_request(
        context(namespace, 602, XByteOrder::LittleEndian),
        &create_gc_request(XByteOrder::LittleEndian, 0x220102, 0x220101),
    )
    .unwrap();
    dispatch_x11_wire_request(
        dispatch_context(namespace, 2, XByteOrder::LittleEndian, 55),
        gc,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );

    let clear = decode_x11_core_request(
        context(namespace, 601, XByteOrder::LittleEndian),
        &clear_area_request(XByteOrder::LittleEndian, false, 0x220101, 4, 5, 33, 22),
    )
    .unwrap();
    let clear = dispatch_x11_wire_request(
        dispatch_context(namespace, 1, XByteOrder::LittleEndian, 61),
        clear,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );

    assert!(clear.outputs.is_empty());
    let response = clear.response.unwrap();
    assert_eq!(response.transactions.len(), 1);
    assert_eq!(
        response.transactions[0].damage,
        Region::single(Rect {
            x: 4,
            y: 5,
            width: 33,
            height: 22,
        })
    );

    let fill = decode_x11_core_request(
        context(namespace, 602, XByteOrder::LittleEndian),
        &poly_fill_rectangle_request(
            XByteOrder::LittleEndian,
            0x220101,
            0x220102,
            &[(5, 6, 40, 30)],
        ),
    )
    .unwrap();
    let fill = dispatch_x11_wire_request(
        dispatch_context(namespace, 2, XByteOrder::LittleEndian, 70),
        fill,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );

    assert!(fill.outputs.is_empty());
    let response = fill.response.unwrap();
    assert_eq!(response.transactions.len(), 1);
    assert_eq!(
        response.transactions[0].surface,
        SurfaceId::new(0x220101, 1)
    );
    assert_eq!(
        response.transactions[0].damage,
        Region::single(Rect {
            x: 5,
            y: 6,
            width: 40,
            height: 30,
        })
    );

    let segments = decode_x11_core_request(
        context(namespace, 603, XByteOrder::LittleEndian),
        &poly_segment_request(
            XByteOrder::LittleEndian,
            0x220101,
            0x220102,
            &[(2, 3, 12, 8)],
        ),
    )
    .unwrap();
    let segments = dispatch_x11_wire_request(
        dispatch_context(namespace, 3, XByteOrder::LittleEndian, 66),
        segments,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );

    assert!(segments.outputs.is_empty());
    let response = segments.response.unwrap();
    assert_eq!(response.transactions.len(), 1);
    assert_eq!(
        response.transactions[0].surface,
        SurfaceId::new(0x220101, 1)
    );
    assert_eq!(
        response.transactions[0].damage,
        Region::single(Rect {
            x: 2,
            y: 3,
            width: 11,
            height: 6,
        })
    );

    let line = decode_x11_core_request(
        context(namespace, 604, XByteOrder::LittleEndian),
        &poly_line_request(
            XByteOrder::LittleEndian,
            0x220101,
            0x220102,
            &[(1, 2), (11, 7), (5, 18)],
        ),
    )
    .unwrap();
    let line = dispatch_x11_wire_request(
        dispatch_context(namespace, 4, XByteOrder::LittleEndian, 65),
        line,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );

    assert!(line.outputs.is_empty());
    let response = line.response.unwrap();
    assert_eq!(response.transactions.len(), 1);
    assert_eq!(
        response.transactions[0].surface,
        SurfaceId::new(0x220101, 1)
    );
    assert_eq!(
        response.transactions[0].damage,
        Region::single(Rect {
            x: 1,
            y: 2,
            width: 11,
            height: 17,
        })
    );

    let fill_poly = decode_x11_core_request(
        context(namespace, 605, XByteOrder::LittleEndian),
        &fill_poly_request(
            XByteOrder::LittleEndian,
            0x220101,
            0x220102,
            &[(4, 5), (14, 10), (7, 20)],
        ),
    )
    .unwrap();
    let fill_poly = dispatch_x11_wire_request(
        dispatch_context(namespace, 5, XByteOrder::LittleEndian, 69),
        fill_poly,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );

    assert!(fill_poly.outputs.is_empty());
    let response = fill_poly.response.unwrap();
    assert_eq!(response.transactions.len(), 1);
    assert_eq!(
        response.transactions[0].surface,
        SurfaceId::new(0x220101, 1)
    );
    assert_eq!(
        response.transactions[0].damage,
        Region::single(Rect {
            x: 4,
            y: 5,
            width: 11,
            height: 16,
        })
    );

    let fill_arcs = decode_x11_core_request(
        context(namespace, 606, XByteOrder::LittleEndian),
        &poly_fill_arc_request(
            XByteOrder::LittleEndian,
            0x220101,
            0x220102,
            &[(6, 7, 22, 12, 0, 23040)],
        ),
    )
    .unwrap();
    let fill_arcs = dispatch_x11_wire_request(
        dispatch_context(namespace, 6, XByteOrder::LittleEndian, 71),
        fill_arcs,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );

    assert!(fill_arcs.outputs.is_empty());
    let response = fill_arcs.response.unwrap();
    assert_eq!(response.transactions.len(), 1);
    assert_eq!(
        response.transactions[0].surface,
        SurfaceId::new(0x220101, 1)
    );
    assert_eq!(
        response.transactions[0].damage,
        Region::single(Rect {
            x: 6,
            y: 7,
            width: 22,
            height: 12,
        })
    );
}

#[test]
fn x11_dispatch_put_image_emits_software_surface_transaction() {
    let namespace = NamespaceId::from_raw(46);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let create = decode_x11_core_request(
        context(namespace, 611, XByteOrder::LittleEndian),
        &create_window_request(XByteOrder::LittleEndian, 0x220111, 10, 20, 640, 480),
    )
    .unwrap();
    dispatch_x11_wire_request(
        dispatch_context(namespace, 1, XByteOrder::LittleEndian, 1),
        create,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );

    let put = decode_x11_core_request(
        context(namespace, 612, XByteOrder::LittleEndian),
        &put_image_request(
            XByteOrder::LittleEndian,
            0x220111,
            0x220112,
            8,
            4,
            3,
            5,
            &[0xaa; 128],
        ),
    )
    .unwrap();
    let put = dispatch_x11_wire_request(
        dispatch_context(namespace, 2, XByteOrder::LittleEndian, 72),
        put,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );

    assert!(put.outputs.is_empty());
    let response = put.response.unwrap();
    assert_eq!(response.transactions.len(), 1);
    assert_eq!(
        response.transactions[0].surface,
        SurfaceId::new(0x220111, 1)
    );
    assert!(matches!(
        response.transactions[0].target_buffer,
        BufferSource::CpuBuffer { .. }
    ));
    assert_eq!(
        response.transactions[0].damage,
        Region::single(Rect {
            x: 3,
            y: 5,
            width: 8,
            height: 4,
        })
    );
}

#[test]
fn x11_dispatch_pixmap_put_image_and_copy_area_emit_window_transaction() {
    let namespace = NamespaceId::from_raw(46);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let create = decode_x11_core_request(
        context(namespace, 621, XByteOrder::LittleEndian),
        &create_window_request(XByteOrder::LittleEndian, 0x220121, 10, 20, 640, 480),
    )
    .unwrap();
    dispatch_x11_wire_request(
        dispatch_context(namespace, 1, XByteOrder::LittleEndian, 1),
        create,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let gc = decode_x11_core_request(
        context(namespace, 622, XByteOrder::LittleEndian),
        &create_gc_request(XByteOrder::LittleEndian, 0x220123, 0x220121),
    )
    .unwrap();
    dispatch_x11_wire_request(
        dispatch_context(namespace, 2, XByteOrder::LittleEndian, 55),
        gc,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );

    let pixmap = decode_x11_core_request(
        context(namespace, 622, XByteOrder::LittleEndian),
        &create_pixmap_request(XByteOrder::LittleEndian, 24, 0x220122, 0x220121, 64, 32),
    )
    .unwrap();
    let pixmap = dispatch_x11_wire_request(
        dispatch_context(namespace, 2, XByteOrder::LittleEndian, 53),
        pixmap,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    assert!(pixmap.outputs.is_empty());

    let put = decode_x11_core_request(
        context(namespace, 623, XByteOrder::LittleEndian),
        &put_image_request(
            XByteOrder::LittleEndian,
            0x220122,
            0x220123,
            8,
            4,
            0,
            0,
            &[0xaa; 128],
        ),
    )
    .unwrap();
    let put = dispatch_x11_wire_request(
        dispatch_context(namespace, 3, XByteOrder::LittleEndian, 72),
        put,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    assert!(put.outputs.is_empty());
    assert!(put.response.unwrap().transactions.is_empty());

    let copy = decode_x11_core_request(
        context(namespace, 624, XByteOrder::LittleEndian),
        &copy_area_request(
            XByteOrder::LittleEndian,
            0x220122,
            0x220121,
            0x220123,
            0,
            0,
            5,
            6,
            8,
            4,
        ),
    )
    .unwrap();
    let copy = dispatch_x11_wire_request(
        dispatch_context(namespace, 4, XByteOrder::LittleEndian, 62),
        copy,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    assert!(copy.outputs.is_empty());
    let response = copy.response.unwrap();
    assert_eq!(response.transactions.len(), 1);
    assert_eq!(
        response.transactions[0].surface,
        SurfaceId::new(0x220121, 1)
    );
    assert_eq!(
        response.transactions[0].damage,
        Region::single(Rect {
            x: 5,
            y: 6,
            width: 8,
            height: 4,
        })
    );
}

#[test]
fn x11_dispatch_accepts_open_and_close_font_resources() {
    let namespace = NamespaceId::from_raw(46);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();

    let open = decode_x11_core_request(
        context(namespace, 631, XByteOrder::LittleEndian),
        &open_font_request(XByteOrder::LittleEndian, 0x220131, "fixed"),
    )
    .unwrap();
    let open = dispatch_x11_wire_request(
        dispatch_context(namespace, 1, XByteOrder::LittleEndian, 45),
        open,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    assert!(open.outputs.is_empty());

    let query = decode_x11_core_request(
        context(namespace, 632, XByteOrder::LittleEndian),
        &resource_request(XByteOrder::LittleEndian, 47, 0x220131),
    )
    .unwrap();
    let query = dispatch_x11_wire_request(
        dispatch_context(namespace, 2, XByteOrder::LittleEndian, 47),
        query,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = query.encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(encoded.len(), 1);
    assert_eq!(encoded[0][0], 1);
    assert_eq!(read_u32(XByteOrder::LittleEndian, &encoded[0][4..8]), 7);
    assert_eq!(read_i16(XByteOrder::LittleEndian, &encoded[0][10..12]), 8);
    assert_eq!(read_i16(XByteOrder::LittleEndian, &encoded[0][12..14]), 8);
    assert_eq!(read_i16(XByteOrder::LittleEndian, &encoded[0][26..28]), 8);
    assert_eq!(read_i16(XByteOrder::LittleEndian, &encoded[0][28..30]), 8);
    assert_eq!(read_u32(XByteOrder::LittleEndian, &encoded[0][56..60]), 0);

    let list = decode_x11_core_request(
        context(namespace, 634, XByteOrder::LittleEndian),
        &list_fonts_request(XByteOrder::LittleEndian, 5, "*"),
    )
    .unwrap();
    let list = dispatch_x11_wire_request(
        dispatch_context(namespace, 3, XByteOrder::LittleEndian, 49),
        list,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = list.encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(encoded.len(), 1);
    assert_eq!(encoded[0][0], 1);
    assert_eq!(read_u16(XByteOrder::LittleEndian, &encoded[0][8..10]), 1);
    assert_eq!(encoded[0][32], 5);
    assert_eq!(&encoded[0][33..38], b"fixed");

    let list = decode_x11_core_request(
        context(namespace, 635, XByteOrder::LittleEndian),
        &list_fonts_with_info_request(XByteOrder::LittleEndian, 5, "*"),
    )
    .unwrap();
    let list = dispatch_x11_wire_request(
        dispatch_context(namespace, 4, XByteOrder::LittleEndian, 50),
        list,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let encoded = list.encoded_outputs(XByteOrder::LittleEndian);
    assert_eq!(encoded.len(), 1);
    assert_eq!(encoded[0][0], 1);
    assert_eq!(encoded[0][1], 5);
    assert_eq!(read_u32(XByteOrder::LittleEndian, &encoded[0][4..8]), 9);
    assert_eq!(&encoded[0][60..65], b"fixed");
    assert_eq!(encoded[0][68], 1);
    assert_eq!(encoded[0][69], 0);
    assert_eq!(read_u32(XByteOrder::LittleEndian, &encoded[0][72..76]), 7);

    let close = decode_x11_core_request(
        context(namespace, 636, XByteOrder::LittleEndian),
        &resource_request(XByteOrder::LittleEndian, 46, 0x220131),
    )
    .unwrap();
    let close = dispatch_x11_wire_request(
        dispatch_context(namespace, 5, XByteOrder::LittleEndian, 46),
        close,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    assert!(close.outputs.is_empty());
}

#[test]
fn x11_dispatch_accepts_glyph_cursor_lifecycle() {
    let namespace = NamespaceId::from_raw(46);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();

    for (sequence, font) in [(1u16, 0x220141), (2u16, 0x220142)] {
        let open = decode_x11_core_request(
            context(
                namespace,
                640 + u64::from(sequence),
                XByteOrder::LittleEndian,
            ),
            &open_font_request(XByteOrder::LittleEndian, font, "cursor"),
        )
        .unwrap();
        let open = dispatch_x11_wire_request(
            dispatch_context(namespace, sequence, XByteOrder::LittleEndian, 45),
            open,
            &mut runtime,
            &mut atoms,
            &mut properties,
        );
        assert!(open.outputs.is_empty());
    }

    let cursor = decode_x11_core_request(
        context(namespace, 643, XByteOrder::LittleEndian),
        &create_glyph_cursor_request(XByteOrder::LittleEndian, 0x220143, 0x220141, 0x220142),
    )
    .unwrap();
    let cursor = dispatch_x11_wire_request(
        dispatch_context(namespace, 3, XByteOrder::LittleEndian, 94),
        cursor,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    assert!(cursor.outputs.is_empty());

    let recolor = decode_x11_core_request(
        context(namespace, 644, XByteOrder::LittleEndian),
        &recolor_cursor_request(XByteOrder::LittleEndian, 0x220143),
    )
    .unwrap();
    let recolor = dispatch_x11_wire_request(
        dispatch_context(namespace, 4, XByteOrder::LittleEndian, 96),
        recolor,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    assert!(recolor.outputs.is_empty());

    let free = decode_x11_core_request(
        context(namespace, 645, XByteOrder::LittleEndian),
        &resource_request(XByteOrder::LittleEndian, 95, 0x220143),
    )
    .unwrap();
    let free = dispatch_x11_wire_request(
        dispatch_context(namespace, 5, XByteOrder::LittleEndian, 95),
        free,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    assert!(free.outputs.is_empty());
}

#[test]
fn x11_dispatch_poly_text8_emits_conservative_text_damage() {
    let namespace = NamespaceId::from_raw(46);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let window = 0x220151;

    let create = decode_x11_core_request(
        context(namespace, 646, XByteOrder::LittleEndian),
        &create_window_request(XByteOrder::LittleEndian, window, 0, 0, 300, 200),
    )
    .unwrap();
    dispatch_x11_wire_request(
        dispatch_context(namespace, 1, XByteOrder::LittleEndian, 1),
        create,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );
    let gc = decode_x11_core_request(
        context(namespace, 647, XByteOrder::LittleEndian),
        &create_gc_request(XByteOrder::LittleEndian, 0x220152, window),
    )
    .unwrap();
    dispatch_x11_wire_request(
        dispatch_context(namespace, 2, XByteOrder::LittleEndian, 55),
        gc,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );

    let text = decode_x11_core_request(
        context(namespace, 648, XByteOrder::LittleEndian),
        &poly_text8_request(XByteOrder::LittleEndian, window, 0x220152, 5, 16, b"Hi"),
    )
    .unwrap();
    let text = dispatch_x11_wire_request(
        dispatch_context(namespace, 3, XByteOrder::LittleEndian, 74),
        text,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );

    let response = text.response.unwrap();
    assert_eq!(response.transactions.len(), 1);
    assert_eq!(
        response.transactions[0].damage,
        Region::single(Rect {
            x: 5,
            y: 6,
            width: 16,
            height: 12,
        })
    );

    let image_text = decode_x11_core_request(
        context(namespace, 649, XByteOrder::LittleEndian),
        &image_text8_request(XByteOrder::LittleEndian, window, 0x220152, 9, 20, b"OK"),
    )
    .unwrap();
    let image_text = dispatch_x11_wire_request(
        dispatch_context(namespace, 4, XByteOrder::LittleEndian, 76),
        image_text,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );

    let response = image_text.response.unwrap();
    assert_eq!(response.transactions.len(), 1);
    assert_eq!(
        response.transactions[0].damage,
        Region::single(Rect {
            x: 9,
            y: 10,
            width: 16,
            height: 12,
        })
    );
}

#[test]
fn x11_dispatch_sophia_present_emits_xpixmap_surface_transaction() {
    let namespace = NamespaceId::from_raw(46);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let create = decode_x11_core_request(
        context(namespace, 621, XByteOrder::LittleEndian),
        &create_window_request(XByteOrder::LittleEndian, 0x220121, 10, 20, 640, 480),
    )
    .unwrap();
    dispatch_x11_wire_request(
        dispatch_context(namespace, 1, XByteOrder::LittleEndian, 1),
        create,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );

    let present = decode_x11_core_request(
        context(namespace, 622, XByteOrder::LittleEndian),
        &sophia_present_pixmap_request(
            XByteOrder::LittleEndian,
            0x220121,
            0x990,
            (3, 5, 32, 24),
            1,
            250,
        ),
    )
    .unwrap();
    let present = dispatch_x11_wire_request(
        dispatch_context(
            namespace,
            2,
            XByteOrder::LittleEndian,
            X_SOPHIA_PRESENT_MAJOR_OPCODE,
        ),
        present,
        &mut runtime,
        &mut atoms,
        &mut properties,
    );

    assert!(present.outputs.is_empty());
    let response = present.response.unwrap();
    assert_eq!(response.transactions.len(), 1);
    assert_eq!(
        response.transactions[0].surface,
        SurfaceId::new(0x220121, 1)
    );
    assert_eq!(
        response.transactions[0].target_buffer,
        BufferSource::XPixmap { pixmap: 0x990 }
    );
    assert_eq!(
        response.transactions[0].damage,
        Region::single(Rect {
            x: 3,
            y: 5,
            width: 32,
            height: 24,
        })
    );
}

#[test]
fn x_authority_transaction_emitter_sends_bounded_batches() {
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    let result = present_dispatch_result(TransactionId::from_raw(610));

    let emitted = try_emit_x_authority_transactions(&sender, &result)
        .unwrap()
        .unwrap();
    let received = receiver.try_recv().unwrap();

    assert_eq!(emitted.transaction, TransactionId::from_raw(610));
    assert_eq!(emitted.transactions.len(), 1);
    assert_eq!(received, emitted);
}

#[test]
fn x_authority_transaction_emitter_reports_backpressure() {
    let (sender, _receiver) = std::sync::mpsc::sync_channel(0);
    let result = present_dispatch_result(TransactionId::from_raw(611));

    assert_eq!(
        try_emit_x_authority_transactions(&sender, &result),
        Err(XAuthorityTransportError::Backpressure {
            transaction: TransactionId::from_raw(611)
        })
    );
}

#[cfg(unix)]
#[test]
fn protocol_router_remains_usable_after_route_broker_moves_or_drops() {
    use std::num::NonZeroUsize;

    let broker = XServerFrontendRouteBroker::new(NonZeroUsize::new(4).unwrap());
    let router = broker.protocol_router();
    let second = router.clone();
    drop(broker);

    assert_eq!(
        router.route_present_complete(
            TransactionId::from_raw(91),
            10,
            20,
            XPresentCompletionMode::Flip,
        ),
        Ok(false)
    );
    assert_eq!(
        second.route_present_idle(TransactionId::from_raw(91)),
        Ok(false)
    );
}

#[cfg(unix)]
#[test]
fn x_server_frontend_config_requires_a_socket_path_and_namespace() {
    assert!(XServerFrontendConfig::new("", NamespaceId::from_raw(1)).is_err());
    assert!(XServerFrontendConfig::new("/tmp/sophia-x11.sock", NamespaceId::INVALID).is_err());

    let config =
        XServerFrontendConfig::new("/tmp/sophia-x11.sock", NamespaceId::from_raw(812)).unwrap();
    assert_eq!(
        config.socket_path(),
        std::path::Path::new("/tmp/sophia-x11.sock")
    );
    assert_eq!(config.namespace(), NamespaceId::from_raw(812));
    assert_eq!(
        config.namespace_context().profile,
        NamespaceProfile::ClassicShared
    );
    assert_eq!(config.max_concurrent_clients().get(), 16);
}

#[cfg(unix)]
#[test]
fn x_server_frontend_config_accepts_a_session_namespace_context() {
    let namespace = NamespaceContext::new(
        NamespaceId::from_raw(821),
        NamespaceProfile::Confined,
        NamespaceCapabilities::NONE
            .with_request(NamespacePortalCapability::Clipboard)
            .with_publish(NamespacePortalCapability::Clipboard),
    )
    .unwrap();

    let config = XServerFrontendConfig::new_with_namespace_context(
        "/tmp/sophia-x11-confined.sock",
        namespace,
    )
    .unwrap();

    assert_eq!(config.namespace(), namespace.id);
    assert_eq!(config.namespace_context(), namespace);
}

#[cfg(unix)]
#[test]
fn x_server_frontend_dri3_open_sends_backend_owned_render_device_fd() {
    use std::fs::File;
    use std::io::{IoSliceMut, Write};
    use std::mem::MaybeUninit;
    use std::os::fd::OwnedFd;
    use std::os::unix::net::UnixStream;
    use std::sync::Arc;
    use std::thread;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TestRenderDeviceProvider;

    impl XServerFrontendRenderDeviceProvider for TestRenderDeviceProvider {
        fn open_render_device_fd(&self) -> Result<OwnedFd, XServerFrontendRenderDeviceError> {
            File::open("/dev/null")
                .map(OwnedFd::from)
                .map_err(|_| XServerFrontendRenderDeviceError::Unavailable)
        }
    }

    let path = std::env::temp_dir().join(format!(
        "sophia-x-server-dri3-open-test-{}-{}.sock",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let config = XServerFrontendConfig::new(&path, NamespaceId::from_raw(822))
        .unwrap()
        .with_render_device_provider(Arc::new(TestRenderDeviceProvider));
    let mut frontend = XServerFrontend::bind(config).unwrap();
    let server = thread::spawn(move || frontend.serve_next());

    wait_for_socket(&path);
    let mut stream = UnixStream::connect(&path).unwrap();
    stream
        .write_all(&setup_request(XByteOrder::LittleEndian, 11, 0, b"", b""))
        .unwrap();
    read_setup_success(&mut stream, XByteOrder::LittleEndian);
    stream
        .write_all(&dri3_open_request(
            XByteOrder::LittleEndian,
            X_SETUP_DEFAULT_ROOT,
            0,
        ))
        .unwrap();

    let mut reply = [0; X_CLIENT_OUTPUT_RECORD_LEN];
    let mut iov = [IoSliceMut::new(&mut reply)];
    let mut ancillary_space = [MaybeUninit::uninit();
        rustix::cmsg_space!(ScmRights(sophia_protocol::DMA_BUF_MAX_PLANES))];
    let mut ancillary = rustix::net::RecvAncillaryBuffer::new(&mut ancillary_space);
    let received = rustix::net::recvmsg(
        &stream,
        &mut iov,
        &mut ancillary,
        rustix::net::RecvFlags::CMSG_CLOEXEC,
    )
    .unwrap();
    assert_eq!(received.bytes, X_CLIENT_OUTPUT_RECORD_LEN);
    assert_eq!(reply[0], 1);
    assert_eq!(reply[1], 1);
    let received_fds = ancillary
        .drain()
        .flat_map(|message| match message {
            rustix::net::RecvAncillaryMessage::ScmRights(fds) => fds.collect::<Vec<_>>(),
            _ => Vec::new(),
        })
        .collect::<Vec<_>>();
    assert_eq!(received_fds.len(), 1);
    File::from(received_fds.into_iter().next().unwrap())
        .metadata()
        .unwrap();

    drop(stream);
    server.join().unwrap().unwrap();
    std::fs::remove_file(path).unwrap();
}

#[cfg(unix)]
#[test]
fn x_server_frontend_assigns_batched_scm_rights_to_fd_bearing_requests() {
    use std::fs::File;
    use std::io::{IoSlice, Write};
    use std::mem::MaybeUninit;
    use std::net::Shutdown;
    use std::os::fd::AsFd;
    use std::os::unix::net::UnixStream;
    use std::thread;
    use std::time::{SystemTime, UNIX_EPOCH};

    let path = std::env::temp_dir().join(format!(
        "sophia-x-server-batched-rights-test-{}-{}.sock",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let config = XServerFrontendConfig::new(&path, NamespaceId::from_raw(823)).unwrap();
    let mut frontend = XServerFrontend::bind(config).unwrap();
    let server = thread::spawn(move || frontend.serve_next());

    wait_for_socket(&path);
    let mut stream = UnixStream::connect(&path).unwrap();
    stream
        .write_all(&setup_request(XByteOrder::LittleEndian, 11, 0, b"", b""))
        .unwrap();
    read_setup_success(&mut stream, XByteOrder::LittleEndian);

    let mut requests = xfixes_create_region_request(XByteOrder::LittleEndian, 0x220810, &[]);
    requests.extend_from_slice(&dri3_pixmap_from_buffer_request(
        XByteOrder::LittleEndian,
        0x220811,
        X_SETUP_DEFAULT_ROOT,
        64 * 48 * 4,
        64,
        48,
        256,
        24,
        32,
    ));
    requests.extend_from_slice(&dri3_fence_from_fd_request(
        XByteOrder::LittleEndian,
        X_SETUP_DEFAULT_ROOT,
        0x220812,
        false,
    ));
    let pixmap_fd = File::open("/dev/null").unwrap();
    let fence_fd = File::open("/dev/null").unwrap();
    let borrowed = [pixmap_fd.as_fd(), fence_fd.as_fd()];
    let mut space = [MaybeUninit::uninit();
        rustix::cmsg_space!(ScmRights(sophia_protocol::DMA_BUF_MAX_PLANES))];
    let mut ancillary = rustix::net::SendAncillaryBuffer::new(&mut space);
    assert!(ancillary.push(rustix::net::SendAncillaryMessage::ScmRights(&borrowed)));
    let sent = rustix::net::sendmsg(
        &stream,
        &[IoSlice::new(&requests)],
        &mut ancillary,
        rustix::net::SendFlags::empty(),
    )
    .unwrap();
    assert_eq!(sent, requests.len());
    stream.shutdown(Shutdown::Write).unwrap();

    server.join().unwrap().unwrap();
    std::fs::remove_file(path).unwrap();
}

#[cfg(unix)]
#[test]
fn x_server_frontend_binds_an_owner_only_socket_and_preserves_regular_files() {
    use std::os::unix::fs::PermissionsExt;
    use std::time::{SystemTime, UNIX_EPOCH};

    let path = std::env::temp_dir().join(format!(
        "sophia-x-server-frontend-test-{}-{}.sock",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let config = XServerFrontendConfig::new(&path, NamespaceId::from_raw(813)).unwrap();
    let frontend = XServerFrontend::bind(config).unwrap();
    assert_eq!(frontend.config().socket_path(), path.as_path());
    assert_eq!(
        std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    drop(frontend);
    std::fs::remove_file(&path).unwrap();

    std::fs::write(&path, b"do not replace regular files").unwrap();
    let config = XServerFrontendConfig::new(&path, NamespaceId::from_raw(814)).unwrap();
    let error = match XServerFrontend::bind(config) {
        Ok(_) => panic!("frontend must not replace a regular file"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("refusing to replace non-socket"));
    assert_eq!(
        std::fs::read(&path).unwrap(),
        b"do not replace regular files"
    );
    std::fs::remove_file(&path).unwrap();
}

#[cfg(unix)]
#[test]
fn x_server_frontend_rejects_bad_cookie_then_accepts_the_configured_cookie() {
    use std::io::{Read, Write};
    use std::os::unix::net::UnixStream;
    use std::sync::Arc;
    use std::thread;
    use std::time::{SystemTime, UNIX_EPOCH};

    let socket_path = std::env::temp_dir().join(format!(
        "sophia-x-server-frontend-cookie-test-{}-{}.sock",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let cookie = [0x3c; 16];
    let namespace = NamespaceContext::new(
        NamespaceId::from_raw(815),
        NamespaceProfile::ClassicShared,
        NamespaceCapabilities::NONE,
    )
    .unwrap();
    let policy = Arc::new(TestXAdmissionPolicy::new(namespace, false));
    let config = XServerFrontendConfig::new_with_namespace_context(&socket_path, namespace)
        .unwrap()
        .with_setup_authorization(XServerFrontendSetupAuthorization::MitMagicCookie(cookie))
        .with_admission_policy(policy.clone());
    assert_eq!(
        format!("{:?}", config.setup_authorization()),
        "MitMagicCookie([redacted])"
    );
    let server = thread::spawn(move || {
        let mut frontend = XServerFrontend::bind(config).unwrap();
        frontend.serve_next().unwrap();
        frontend.serve_next().unwrap();
    });

    wait_for_socket(&socket_path);
    let mut rejected = UnixStream::connect(&socket_path).unwrap();
    rejected
        .write_all(&setup_request(
            XByteOrder::LittleEndian,
            11,
            0,
            b"MIT-MAGIC-COOKIE-1",
            b"wrong-cookie-data",
        ))
        .unwrap();
    let mut rejected_prefix = [0; X_SETUP_REPLY_PREFIX_LEN];
    rejected.read_exact(&mut rejected_prefix).unwrap();
    assert_eq!(rejected_prefix[0], 0);
    let rejected_body_len =
        usize::from(read_u16(XByteOrder::LittleEndian, &rejected_prefix[6..8])) * 4;
    let mut rejected_body = vec![0; rejected_body_len];
    rejected.read_exact(&mut rejected_body).unwrap();
    assert!(String::from_utf8_lossy(&rejected_body).contains("authorization failed"));
    drop(rejected);

    let mut accepted = UnixStream::connect(&socket_path).unwrap();
    accepted
        .write_all(&setup_request(
            XByteOrder::LittleEndian,
            11,
            0,
            b"MIT-MAGIC-COOKIE-1",
            &cookie,
        ))
        .unwrap();
    read_setup_success(&mut accepted, XByteOrder::LittleEndian);
    drop(accepted);

    server.join().unwrap();
    let requests = policy.requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(
        requests[0].setup_authentication,
        ClientAuthenticationMethod::MitMagicCookie1
    );
    assert_eq!(policy.revoked.lock().unwrap().len(), 1);
    std::fs::remove_file(&socket_path).unwrap();
}

#[cfg(unix)]
struct TestXAdmissionPolicy {
    namespace: NamespaceContext,
    deny: bool,
    next_client: std::sync::atomic::AtomicU64,
    requests: std::sync::Mutex<Vec<XServerFrontendAdmissionRequest>>,
    revoked: std::sync::Mutex<Vec<ClientAdmissionContext>>,
}

#[cfg(unix)]
impl TestXAdmissionPolicy {
    fn new(namespace: NamespaceContext, deny: bool) -> Self {
        Self {
            namespace,
            deny,
            next_client: std::sync::atomic::AtomicU64::new(1),
            requests: std::sync::Mutex::new(Vec::new()),
            revoked: std::sync::Mutex::new(Vec::new()),
        }
    }
}

#[cfg(unix)]
impl XServerFrontendAdmissionPolicy for TestXAdmissionPolicy {
    fn admit(
        &self,
        request: XServerFrontendAdmissionRequest,
    ) -> Result<ClientAdmissionContext, XServerFrontendAdmissionError> {
        self.requests.lock().unwrap().push(request);
        if self.deny {
            return Err(XServerFrontendAdmissionError::Denied);
        }
        let client = self
            .next_client
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Ok(ClientAdmissionContext::new(
            ClientAdmissionId::from_raw(client),
            self.namespace,
            ClientAuthProvenance::new(ClientAuthenticationMethod::PeerCredentials, 7).unwrap(),
        )
        .unwrap())
    }

    fn revoke(&self, context: ClientAdmissionContext) -> Result<(), XServerFrontendAdmissionError> {
        self.revoked.lock().unwrap().push(context);
        Ok(())
    }
}

#[cfg(unix)]
struct SequencedXAdmissionPolicy {
    namespaces: [NamespaceContext; 2],
    next_client: std::sync::atomic::AtomicU64,
    revoked: std::sync::Mutex<Vec<ClientAdmissionContext>>,
}

#[cfg(unix)]
impl XServerFrontendAdmissionPolicy for SequencedXAdmissionPolicy {
    fn admit(
        &self,
        request: XServerFrontendAdmissionRequest,
    ) -> Result<ClientAdmissionContext, XServerFrontendAdmissionError> {
        let index = self
            .next_client
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let namespace = self
            .namespaces
            .get(usize::try_from(index).map_err(|_| XServerFrontendAdmissionError::Unavailable)?)
            .copied()
            .ok_or(XServerFrontendAdmissionError::Unavailable)?;
        ClientAdmissionContext::new(
            ClientAdmissionId::from_raw(index + 1),
            namespace,
            ClientAuthProvenance::new(request.setup_authentication, 9).unwrap(),
        )
        .ok_or(XServerFrontendAdmissionError::Unavailable)
    }

    fn revoke(&self, context: ClientAdmissionContext) -> Result<(), XServerFrontendAdmissionError> {
        self.revoked.lock().unwrap().push(context);
        Ok(())
    }
}

#[cfg(unix)]
#[test]
fn x_server_frontend_reports_admission_denial_as_x11_setup_failure() {
    use std::io::{Read, Write};
    use std::os::unix::net::UnixStream;
    use std::sync::Arc;
    use std::thread;
    use std::time::{SystemTime, UNIX_EPOCH};

    let socket_path = std::env::temp_dir().join(format!(
        "sophia-x-server-admission-denial-test-{}-{}.sock",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let namespace = NamespaceContext::new(
        NamespaceId::from_raw(825),
        NamespaceProfile::ClassicShared,
        NamespaceCapabilities::NONE,
    )
    .unwrap();
    let policy = Arc::new(TestXAdmissionPolicy::new(namespace, true));
    let config = XServerFrontendConfig::new_with_namespace_context(&socket_path, namespace)
        .unwrap()
        .with_admission_policy(policy.clone());
    let server = thread::spawn(move || {
        let mut frontend = XServerFrontend::bind(config).unwrap();
        frontend.serve_next().unwrap();
    });

    wait_for_socket(&socket_path);
    let mut client = UnixStream::connect(&socket_path).unwrap();
    client
        .write_all(&setup_request(XByteOrder::LittleEndian, 11, 0, b"", b""))
        .unwrap();
    let mut prefix = [0; X_SETUP_REPLY_PREFIX_LEN];
    client.read_exact(&mut prefix).unwrap();
    assert_eq!(prefix[0], 0);
    let body_len = usize::from(read_u16(XByteOrder::LittleEndian, &prefix[6..8])) * 4;
    let mut body = vec![0; body_len];
    client.read_exact(&mut body).unwrap();
    assert!(String::from_utf8_lossy(&body).contains("admission failed"));
    drop(client);

    server.join().unwrap();
    let requests = policy.requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(
        requests[0].setup_authentication,
        ClientAuthenticationMethod::TrustedLocal
    );
    assert!(requests[0].peer_credentials.is_some());
    assert!(policy.revoked.lock().unwrap().is_empty());
    std::fs::remove_file(&socket_path).unwrap();
}

#[cfg(unix)]
#[test]
fn x_server_frontend_revokes_distinct_admissions_for_concurrent_clients() {
    use std::io::Write;
    use std::num::NonZeroUsize;
    use std::os::unix::net::UnixStream;
    use std::sync::Arc;
    use std::thread;
    use std::time::{SystemTime, UNIX_EPOCH};

    let socket_path = std::env::temp_dir().join(format!(
        "sophia-x-server-admission-concurrency-test-{}-{}.sock",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let namespace = NamespaceContext::new(
        NamespaceId::from_raw(826),
        NamespaceProfile::ClassicShared,
        NamespaceCapabilities::NONE,
    )
    .unwrap();
    let policy = Arc::new(TestXAdmissionPolicy::new(namespace, false));
    let config = XServerFrontendConfig::new_with_namespace_context(&socket_path, namespace)
        .unwrap()
        .with_admission_policy(policy.clone())
        .with_max_concurrent_clients(NonZeroUsize::new(2).unwrap());
    let server = thread::spawn(move || {
        let mut frontend = XServerFrontend::bind(config).unwrap();
        frontend.serve_next_concurrently().unwrap();
        frontend.serve_next_concurrently().unwrap();
        frontend.wait_for_clients().unwrap();
        frontend.active_client_count()
    });

    wait_for_socket(&socket_path);
    let mut first = UnixStream::connect(&socket_path).unwrap();
    first
        .write_all(&setup_request(XByteOrder::LittleEndian, 11, 0, b"", b""))
        .unwrap();
    read_setup_success(&mut first, XByteOrder::LittleEndian);
    let mut second = UnixStream::connect(&socket_path).unwrap();
    second
        .write_all(&setup_request(XByteOrder::LittleEndian, 11, 0, b"", b""))
        .unwrap();
    read_setup_success(&mut second, XByteOrder::LittleEndian);

    assert_eq!(policy.requests.lock().unwrap().len(), 2);
    drop(first);
    drop(second);
    assert_eq!(server.join().unwrap(), 0);

    let revoked = policy.revoked.lock().unwrap();
    assert_eq!(revoked.len(), 2);
    assert_ne!(revoked[0].client_id, revoked[1].client_id);
    assert!(revoked.iter().all(|context| context.namespace == namespace));
    std::fs::remove_file(&socket_path).unwrap();
}

#[cfg(unix)]
#[test]
fn x_server_frontend_confined_clients_reject_cross_namespace_window_property_and_selection_access()
{
    use std::io::{Read, Write};
    use std::num::NonZeroUsize;
    use std::os::unix::net::UnixStream;
    use std::sync::Arc;
    use std::thread;
    use std::time::{SystemTime, UNIX_EPOCH};

    let socket_path = std::env::temp_dir().join(format!(
        "sophia-x-server-confined-namespace-test-{}-{}.sock",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let first_namespace = NamespaceContext::new(
        NamespaceId::from_raw(828),
        NamespaceProfile::Confined,
        NamespaceCapabilities::NONE,
    )
    .unwrap();
    let second_namespace = NamespaceContext::new(
        NamespaceId::from_raw(829),
        NamespaceProfile::Confined,
        NamespaceCapabilities::NONE,
    )
    .unwrap();
    let policy = Arc::new(SequencedXAdmissionPolicy {
        namespaces: [first_namespace, second_namespace],
        next_client: std::sync::atomic::AtomicU64::new(0),
        revoked: std::sync::Mutex::new(Vec::new()),
    });
    let metadata_candidates = Arc::new(std::sync::Mutex::new(0usize));
    let config = XServerFrontendConfig::new_with_namespace_context(&socket_path, first_namespace)
        .unwrap()
        .with_admission_policy(policy.clone())
        .with_max_concurrent_clients(NonZeroUsize::new(2).unwrap());
    let server_metadata_candidates = metadata_candidates.clone();
    let server = thread::spawn(move || {
        let mut frontend = XServerFrontend::bind(config).unwrap();
        let observer: Arc<X11CoreTraceObserver> = Arc::new(move |trace| {
            let mut count = server_metadata_candidates.lock().unwrap();
            *count = count.saturating_add(trace.result.metadata_candidates.len());
            Ok(())
        });
        frontend
            .serve_next_concurrently_traced(observer.clone())
            .unwrap();
        frontend.serve_next_concurrently_traced(observer).unwrap();
        frontend.wait_for_clients().unwrap();
    });

    wait_for_socket(&socket_path);
    let mut first = UnixStream::connect(&socket_path).unwrap();
    first
        .write_all(&setup_request(XByteOrder::LittleEndian, 11, 0, b"", b""))
        .unwrap();
    assert_eq!(
        read_setup_resource_id_base(&mut first, XByteOrder::LittleEndian),
        X_SETUP_DEFAULT_RESOURCE_ID_BASE
    );
    let first_window = X_SETUP_DEFAULT_RESOURCE_ID_BASE + 1;
    first
        .write_all(&create_window_request(
            XByteOrder::LittleEndian,
            first_window,
            0,
            0,
            160,
            90,
        ))
        .unwrap();
    assert_eq!(read_x_record(&mut first)[0], 22);

    let mut second = UnixStream::connect(&socket_path).unwrap();
    second
        .write_all(&setup_request(XByteOrder::LittleEndian, 11, 0, b"", b""))
        .unwrap();
    assert_eq!(
        read_setup_resource_id_base(&mut second, XByteOrder::LittleEndian),
        0x0040_0000
    );
    second
        .write_all(&resource_request(XByteOrder::LittleEndian, 8, first_window))
        .unwrap();
    let mut error = [0; 32];
    second.read_exact(&mut error).unwrap();
    assert_eq!(error[0], 0);
    assert_eq!(error[1], XErrorCode::BadAccess.wire_code());

    second
        .write_all(&change_window_event_mask_request(
            XByteOrder::LittleEndian,
            first_window,
            (1 << 0) | (1 << 1),
        ))
        .unwrap();
    second.read_exact(&mut error).unwrap();
    assert_eq!(error[0], 0);
    assert_eq!(error[1], XErrorCode::BadAccess.wire_code());

    second
        .write_all(&change_property_request(
            XByteOrder::LittleEndian,
            XPropertyMode::Replace,
            first_window,
            X_ATOM_WM_NAME,
            X_ATOM_STRING,
            8,
            b"foreign title",
        ))
        .unwrap();
    second.read_exact(&mut error).unwrap();
    assert_eq!(error[0], 0);
    assert_eq!(error[1], XErrorCode::BadAccess.wire_code());

    second
        .write_all(&set_selection_owner_request(
            XByteOrder::LittleEndian,
            first_window,
            X_ATOM_PRIMARY,
            1,
        ))
        .unwrap();
    second.read_exact(&mut error).unwrap();
    assert_eq!(error[0], 0);
    assert_eq!(error[1], XErrorCode::BadAccess.wire_code());

    second
        .write_all(&convert_selection_request(
            XByteOrder::LittleEndian,
            first_window,
            X_ATOM_PRIMARY,
            X_ATOM_STRING,
            X_ATOM_WM_NAME,
            2,
        ))
        .unwrap();
    second.read_exact(&mut error).unwrap();
    assert_eq!(error[0], 31);
    assert_eq!(read_u32(XByteOrder::LittleEndian, &error[20..24]), 0);

    drop(first);
    drop(second);
    server.join().unwrap();
    let revoked = policy.revoked.lock().unwrap();
    assert_eq!(revoked.len(), 2);
    assert!(
        revoked
            .iter()
            .all(|context| context.namespace.profile == NamespaceProfile::Confined)
    );
    assert_ne!(revoked[0].namespace.id, revoked[1].namespace.id);
    assert_eq!(*metadata_candidates.lock().unwrap(), 0);
    std::fs::remove_file(&socket_path).unwrap();
}

#[cfg(unix)]
#[test]
fn x_server_frontend_revokes_admission_after_dispatch_failure() {
    use std::io::Write;
    use std::os::unix::net::UnixStream;
    use std::sync::Arc;
    use std::thread;
    use std::time::{SystemTime, UNIX_EPOCH};

    let socket_path = std::env::temp_dir().join(format!(
        "sophia-x-server-admission-error-cleanup-test-{}-{}.sock",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let namespace = NamespaceContext::new(
        NamespaceId::from_raw(827),
        NamespaceProfile::ClassicShared,
        NamespaceCapabilities::NONE,
    )
    .unwrap();
    let policy = Arc::new(TestXAdmissionPolicy::new(namespace, false));
    let config = XServerFrontendConfig::new_with_namespace_context(&socket_path, namespace)
        .unwrap()
        .with_admission_policy(policy.clone());
    let server = thread::spawn(move || {
        let mut frontend = XServerFrontend::bind(config).unwrap();
        let error = frontend
            .serve_next_traced(|_| Err(X11SetupSocketError::new("injected observer failure")))
            .unwrap_err();
        (error.to_string(), frontend.active_client_count())
    });

    wait_for_socket(&socket_path);
    let mut client = UnixStream::connect(&socket_path).unwrap();
    client
        .write_all(&setup_request(XByteOrder::LittleEndian, 11, 0, b"", b""))
        .unwrap();
    read_setup_success(&mut client, XByteOrder::LittleEndian);
    client
        .write_all(&intern_atom_request(
            XByteOrder::LittleEndian,
            false,
            "FORCE_OBSERVER_FAILURE",
        ))
        .unwrap();

    let (error, active_clients) = server.join().unwrap();
    assert_eq!(error, "injected observer failure");
    assert_eq!(active_clients, 0);
    assert_eq!(policy.revoked.lock().unwrap().len(), 1);
    drop(client);
    std::fs::remove_file(&socket_path).unwrap();
}

#[cfg(unix)]
#[test]
fn x_server_frontend_assigns_disjoint_setup_resource_ranges_to_clients() {
    use std::io::Write;
    use std::os::unix::net::UnixStream;
    use std::thread;
    use std::time::{SystemTime, UNIX_EPOCH};

    let socket_path = std::env::temp_dir().join(format!(
        "sophia-x-server-frontend-resource-ranges-test-{}-{}.sock",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let config = XServerFrontendConfig::new(&socket_path, NamespaceId::from_raw(816)).unwrap();
    let server = thread::spawn(move || {
        let mut frontend = XServerFrontend::bind(config).unwrap();
        frontend.serve_next().unwrap();
        frontend.serve_next().unwrap();
    });

    wait_for_socket(&socket_path);
    let mut first = UnixStream::connect(&socket_path).unwrap();
    first
        .write_all(&setup_request(XByteOrder::LittleEndian, 11, 0, b"", b""))
        .unwrap();
    let first_base = read_setup_resource_id_base(&mut first, XByteOrder::LittleEndian);
    drop(first);

    let mut second = UnixStream::connect(&socket_path).unwrap();
    second
        .write_all(&setup_request(XByteOrder::LittleEndian, 11, 0, b"", b""))
        .unwrap();
    let second_base = read_setup_resource_id_base(&mut second, XByteOrder::LittleEndian);
    drop(second);

    assert_eq!(first_base, X_SETUP_DEFAULT_RESOURCE_ID_BASE);
    assert_eq!(second_base, 0x0040_0000);
    assert_eq!(
        second_base - first_base,
        X_SETUP_DEFAULT_RESOURCE_ID_MASK + 1
    );
    server.join().unwrap();
    std::fs::remove_file(&socket_path).unwrap();
}

#[cfg(unix)]
#[test]
fn x_server_frontend_routes_selection_notify_to_the_requestor_client() {
    use std::io::{Read, Write};
    use std::net::Shutdown;
    use std::num::NonZeroUsize;
    use std::os::unix::net::UnixStream;
    use std::sync::Arc;
    use std::thread;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    let socket_path = std::env::temp_dir().join(format!(
        "sophia-x-server-frontend-selection-route-test-{}-{}.sock",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let config = XServerFrontendConfig::new(&socket_path, NamespaceId::from_raw(817))
        .unwrap()
        .with_max_concurrent_clients(NonZeroUsize::new(2).unwrap());
    let server = thread::spawn(move || {
        let broker = XServerFrontendRouteBroker::new(NonZeroUsize::new(4).unwrap());
        let mut frontend = XServerFrontend::bind(config).unwrap();
        let observer: Arc<X11CoreTraceObserver> = Arc::new(|_| Ok(()));
        frontend
            .serve_next_concurrently_routed_traced(&broker, observer.clone())
            .unwrap();
        frontend
            .serve_next_concurrently_routed_traced(&broker, observer)
            .unwrap();
        frontend.wait_for_clients().unwrap();
    });

    wait_for_socket(&socket_path);
    let mut owner = UnixStream::connect(&socket_path).unwrap();
    owner
        .write_all(&setup_request(XByteOrder::LittleEndian, 11, 0, b"", b""))
        .unwrap();
    let owner_window = read_setup_resource_id_base(&mut owner, XByteOrder::LittleEndian) + 1;
    owner
        .write_all(&create_window_request(
            XByteOrder::LittleEndian,
            owner_window,
            0,
            0,
            160,
            90,
        ))
        .unwrap();
    assert_eq!(read_x_record(&mut owner)[0], 22);
    owner
        .write_all(&set_selection_owner_request(
            XByteOrder::LittleEndian,
            owner_window,
            1,
            10,
        ))
        .unwrap();

    let mut requestor = UnixStream::connect(&socket_path).unwrap();
    requestor
        .write_all(&setup_request(XByteOrder::LittleEndian, 11, 0, b"", b""))
        .unwrap();
    let requestor_window =
        read_setup_resource_id_base(&mut requestor, XByteOrder::LittleEndian) + 1;
    requestor
        .write_all(&create_window_request(
            XByteOrder::LittleEndian,
            requestor_window,
            0,
            0,
            160,
            90,
        ))
        .unwrap();
    assert_eq!(read_x_record(&mut requestor)[0], 22);

    requestor
        .write_all(&convert_selection_request(
            XByteOrder::LittleEndian,
            requestor_window,
            1,
            31,
            31,
            10,
        ))
        .unwrap();
    let request = read_x_record(&mut owner);
    assert_eq!(request[0], 30);
    assert_eq!(read_u16(XByteOrder::LittleEndian, &request[2..4]), 2);
    assert_eq!(
        read_u32(XByteOrder::LittleEndian, &request[8..12]),
        owner_window
    );
    assert_eq!(
        read_u32(XByteOrder::LittleEndian, &request[12..16]),
        requestor_window
    );
    assert_eq!(read_u32(XByteOrder::LittleEndian, &request[16..20]), 1);
    assert_eq!(read_u32(XByteOrder::LittleEndian, &request[20..24]), 31);
    assert_eq!(read_u32(XByteOrder::LittleEndian, &request[24..28]), 31);

    owner
        .write_all(&send_selection_notify_request(
            XByteOrder::LittleEndian,
            requestor_window,
            10,
            11,
            12,
            13,
        ))
        .unwrap();
    let event = read_x_record(&mut requestor);
    assert_eq!(event[0] & 0x7f, 31);
    assert_eq!(read_u16(XByteOrder::LittleEndian, &event[2..4]), 2);
    assert_eq!(
        read_u32(XByteOrder::LittleEndian, &event[8..12]),
        requestor_window
    );
    assert_eq!(read_u32(XByteOrder::LittleEndian, &event[12..16]), 11);
    assert_eq!(read_u32(XByteOrder::LittleEndian, &event[16..20]), 12);
    assert_eq!(read_u32(XByteOrder::LittleEndian, &event[20..24]), 13);

    requestor
        .write_all(&set_selection_owner_request(
            XByteOrder::LittleEndian,
            requestor_window,
            1,
            11,
        ))
        .unwrap();
    let clear = read_x_record(&mut owner);
    assert_eq!(clear[0], 29);
    assert_eq!(read_u16(XByteOrder::LittleEndian, &clear[2..4]), 3);
    assert_eq!(
        read_u32(XByteOrder::LittleEndian, &clear[8..12]),
        owner_window
    );
    assert_eq!(read_u32(XByteOrder::LittleEndian, &clear[12..16]), 1);

    owner
        .set_read_timeout(Some(Duration::from_millis(20)))
        .unwrap();
    let mut unexpected = [0; 1];
    assert!(owner.read(&mut unexpected).is_err());
    owner.shutdown(Shutdown::Both).unwrap();
    requestor.shutdown(Shutdown::Both).unwrap();
    server.join().unwrap();
    std::fs::remove_file(&socket_path).unwrap();
}

#[cfg(unix)]
#[test]
fn cross_namespace_executor_installs_property_and_notifies_requestor() {
    use std::io::Write;
    use std::net::Shutdown;
    use std::num::NonZeroUsize;
    use std::os::unix::net::UnixStream;
    use std::sync::Arc;
    use std::thread;
    use std::time::{SystemTime, UNIX_EPOCH};

    let socket_path = std::env::temp_dir().join(format!(
        "sophia-x11-cross-selection-test-{}-{}.sock",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let portal_path = socket_path.with_extension("portal.sock");
    let source = NamespaceContext::new(
        NamespaceId::from_raw(860),
        NamespaceProfile::Confined,
        NamespaceCapabilities::NONE,
    )
    .unwrap();
    let target = NamespaceContext::new(
        NamespaceId::from_raw(861),
        NamespaceProfile::Confined,
        NamespaceCapabilities::NONE,
    )
    .unwrap();
    let policy = Arc::new(SequencedXAdmissionPolicy {
        namespaces: [source, target],
        next_client: std::sync::atomic::AtomicU64::new(0),
        revoked: std::sync::Mutex::new(Vec::new()),
    });
    let config = XServerFrontendConfig::new_with_namespace_context(&socket_path, source)
        .unwrap()
        .with_admission_policy(policy)
        .with_max_concurrent_clients(NonZeroUsize::new(2).unwrap());
    let (executor_sender, executor_receiver) = std::sync::mpsc::sync_channel(1);
    let (request_sender, request_receiver) = std::sync::mpsc::sync_channel(1);
    let (coordinate_sender, coordinate_receiver) = std::sync::mpsc::sync_channel(1);
    let server_portal_path = portal_path.clone();
    let server = thread::spawn(move || {
        let broker = XServerFrontendRouteBroker::new(NonZeroUsize::new(4).unwrap());
        let mut frontend = XServerFrontend::bind(config).unwrap();
        executor_sender
            .send(frontend.clipboard_executor(&broker))
            .unwrap();
        let first_request = Arc::new(std::sync::atomic::AtomicBool::new(true));
        let observer: Arc<X11CoreTraceObserver> = Arc::new(move |trace| {
            if trace
                .request_detail
                .as_deref()
                .is_some_and(|detail| detail.starts_with("RequestSelection:"))
            {
                request_sender.send(()).unwrap();
                if first_request.swap(false, std::sync::atomic::Ordering::AcqRel) {
                    coordinate_sender.send(()).unwrap();
                }
            }
            Ok(())
        });
        frontend
            .serve_next_concurrently_routed_traced(&broker, observer.clone())
            .unwrap();
        frontend
            .serve_next_concurrently_routed_traced(&broker, observer)
            .unwrap();
        coordinate_receiver.recv().unwrap();
        let request = PortalBrokerRequestPacket {
            request: PortalRequest {
                transfer: PortalTransfer {
                    transfer: PortalTransferId::from_raw(2),
                    source_namespace: source.id,
                    target_namespace: target.id,
                    kind: PortalTransferKind::Clipboard,
                    mime_type: Some("UTF8_STRING".to_owned()),
                    byte_size: 0,
                    decision: PortalDecision::Pending,
                    generation: 1,
                },
                deadline_msec: 2_000,
            },
            source_may_publish: true,
            target_may_request: true,
        };
        coordinate_x11_clipboard_transfer(
            server_portal_path,
            &request,
            &frontend.clipboard_executor(&broker),
            &broker,
            std::time::Duration::from_secs(2),
        )
        .unwrap();
        frontend.wait_for_clients().unwrap();
    });
    wait_for_socket(&socket_path);
    let executor = executor_receiver.recv().unwrap();
    let portal_executor = executor.clone();
    let portal_server_path = portal_path.clone();
    let portal_server = thread::spawn(move || {
        sophia_portal::run_portal_clipboard_broker_socket_server_bounded(
            portal_server_path,
            1,
            sophia_portal::HeadlessPortalPolicy::Allow,
            10,
            1,
            move |grant, payload| {
                portal_executor
                    .execute(grant, payload)
                    .map(|_| ())
                    .map_err(|_| ())
            },
        )
    });
    wait_for_socket(&portal_path);
    let mut owner = UnixStream::connect(&socket_path).unwrap();
    owner
        .write_all(&setup_request(XByteOrder::LittleEndian, 11, 0, b"", b""))
        .unwrap();
    let owner_window = read_setup_resource_id_base(&mut owner, XByteOrder::LittleEndian) + 1;
    owner
        .write_all(&create_window_request(
            XByteOrder::LittleEndian,
            owner_window,
            0,
            0,
            100,
            60,
        ))
        .unwrap();
    read_x_record(&mut owner);
    owner
        .write_all(&intern_atom_request(
            XByteOrder::LittleEndian,
            false,
            "UTF8_STRING",
        ))
        .unwrap();
    let atom_reply = read_x_record(&mut owner);
    let utf8 = read_u32(XByteOrder::LittleEndian, &atom_reply[8..12]);
    owner
        .write_all(&intern_atom_request(
            XByteOrder::LittleEndian,
            false,
            "CLIPBOARD",
        ))
        .unwrap();
    let selection = read_u32(XByteOrder::LittleEndian, &read_x_record(&mut owner)[8..12]);
    owner
        .write_all(&set_selection_owner_request(
            XByteOrder::LittleEndian,
            owner_window,
            selection,
            10,
        ))
        .unwrap();

    let mut requestor = UnixStream::connect(&socket_path).unwrap();
    requestor
        .write_all(&setup_request(XByteOrder::LittleEndian, 11, 0, b"", b""))
        .unwrap();
    let requestor_window =
        read_setup_resource_id_base(&mut requestor, XByteOrder::LittleEndian) + 1;
    requestor
        .write_all(&create_window_request(
            XByteOrder::LittleEndian,
            requestor_window,
            0,
            0,
            100,
            60,
        ))
        .unwrap();
    read_x_record(&mut requestor);
    requestor
        .write_all(&convert_selection_request(
            XByteOrder::LittleEndian,
            requestor_window,
            selection,
            utf8,
            utf8,
            11,
        ))
        .unwrap();
    request_receiver.recv().unwrap();
    let source_request = read_x_record(&mut owner);
    assert_eq!(source_request[0], 30);
    let proxy = read_u32(XByteOrder::LittleEndian, &source_request[12..16]);
    let proxy_property = read_u32(XByteOrder::LittleEndian, &source_request[24..28]);
    owner
        .write_all(&change_property_request(
            XByteOrder::LittleEndian,
            XPropertyMode::Replace,
            proxy,
            proxy_property,
            utf8,
            8,
            b"cross namespace",
        ))
        .unwrap();
    assert_eq!(read_x_record(&mut owner)[0], 28);
    owner
        .write_all(&send_selection_notify_request(
            XByteOrder::LittleEndian,
            proxy,
            read_u32(XByteOrder::LittleEndian, &source_request[4..8]),
            read_u32(XByteOrder::LittleEndian, &source_request[16..20]),
            read_u32(XByteOrder::LittleEndian, &source_request[20..24]),
            proxy_property,
        ))
        .unwrap();
    let notify = read_x_record(&mut requestor);
    assert_eq!(notify[0], 31);
    assert_eq!(read_u32(XByteOrder::LittleEndian, &notify[20..24]), utf8);
    requestor
        .write_all(&get_property_request(
            XByteOrder::LittleEndian,
            false,
            requestor_window,
            utf8,
            utf8,
            0,
            64,
        ))
        .unwrap();
    let reply = read_x_reply(&mut requestor, XByteOrder::LittleEndian);
    assert_eq!(&reply[32..47], b"cross namespace");
    portal_server.join().unwrap().unwrap();

    requestor
        .write_all(&convert_selection_request(
            XByteOrder::LittleEndian,
            requestor_window,
            selection,
            utf8,
            utf8,
            12,
        ))
        .unwrap();
    request_receiver.recv().unwrap();
    owner
        .write_all(&set_selection_owner_request(
            XByteOrder::LittleEndian,
            owner_window,
            selection,
            12,
        ))
        .unwrap();
    owner
        .write_all(&resource_request(XByteOrder::LittleEndian, 23, selection))
        .unwrap();
    assert_eq!(read_x_record(&mut owner)[0], 1);
    let stale_transfer = PortalTransferId::from_raw(4);
    assert!(
        executor
            .request_source(&PortalGrant {
                transfer: stale_transfer,
                source_namespace: source.id,
                target_namespace: target.id,
                kind: PortalTransferKind::Clipboard,
                source_generation: 1,
                broker_generation: 1,
                deadline_msec: 2_000,
                state: PortalGrantState::Active,
            })
            .is_err()
    );
    executor
        .fail(
            stale_transfer,
            ClipboardSelectionExecutionError::StaleOwnerGeneration,
        )
        .unwrap();
    let notify = read_x_record(&mut requestor);
    assert_eq!(notify[0], 31);
    assert_eq!(read_u32(XByteOrder::LittleEndian, &notify[20..24]), 0);

    for (sequence, failure) in [
        (5, ClipboardSelectionExecutionError::Denied),
        (6, ClipboardSelectionExecutionError::Expired),
        (7, ClipboardSelectionExecutionError::Disconnected),
        (8, ClipboardSelectionExecutionError::ExecutorFailure),
    ] {
        requestor
            .write_all(&convert_selection_request(
                XByteOrder::LittleEndian,
                requestor_window,
                selection,
                utf8,
                utf8,
                12,
            ))
            .unwrap();
        request_receiver.recv().unwrap();
        let outcome = executor
            .fail(PortalTransferId::from_raw(sequence), failure)
            .unwrap();
        assert!(matches!(
            outcome,
            ClipboardSelectionExecutionOutcome::Failed { error, .. } if error == failure
        ));
        let notify = read_x_record(&mut requestor);
        assert_eq!(notify[0], 31);
        assert_eq!(read_u32(XByteOrder::LittleEndian, &notify[20..24]), 0);
    }
    owner.shutdown(Shutdown::Both).unwrap();
    requestor.shutdown(Shutdown::Both).unwrap();
    server.join().unwrap();
    std::fs::remove_file(&socket_path).unwrap();
}

#[cfg(unix)]
#[test]
fn x_server_frontend_assigns_distinct_connection_identities() {
    use std::io::Write;
    use std::os::unix::net::UnixStream;
    use std::thread;
    use std::time::{SystemTime, UNIX_EPOCH};

    let socket_path = std::env::temp_dir().join(format!(
        "sophia-x-server-frontend-client-id-test-{}-{}.sock",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let config = XServerFrontendConfig::new(&socket_path, NamespaceId::from_raw(818)).unwrap();
    let server = thread::spawn(move || {
        let mut frontend = XServerFrontend::bind(config).unwrap();
        let mut clients = Vec::new();
        for _ in 0..2 {
            frontend
                .serve_next_traced(|trace| {
                    clients.push((trace.client.raw(), trace.resource_id_range));
                    Ok(())
                })
                .unwrap();
        }
        (clients, frontend.active_client_count())
    });

    wait_for_socket(&socket_path);
    for name in ["FIRST_CLIENT", "SECOND_CLIENT"] {
        let mut stream = UnixStream::connect(&socket_path).unwrap();
        stream
            .write_all(&setup_request(XByteOrder::LittleEndian, 11, 0, b"", b""))
            .unwrap();
        read_setup_success(&mut stream, XByteOrder::LittleEndian);
        stream
            .write_all(&intern_atom_request(XByteOrder::LittleEndian, false, name))
            .unwrap();
        let reply = read_x_record(&mut stream);
        assert_eq!(reply[0], 1);
        drop(stream);
    }

    assert_eq!(
        server.join().unwrap(),
        (
            vec![
                (
                    1,
                    XWireClientResourceRange {
                        base: 0x0020_0000,
                        mask: X_SETUP_DEFAULT_RESOURCE_ID_MASK,
                    },
                ),
                (
                    2,
                    XWireClientResourceRange {
                        base: 0x0040_0000,
                        mask: X_SETUP_DEFAULT_RESOURCE_ID_MASK,
                    },
                ),
            ],
            0,
        )
    );
    std::fs::remove_file(&socket_path).unwrap();
}

#[cfg(unix)]
#[test]
fn x_server_frontend_dispatches_two_live_clients_with_shared_x_state() {
    use std::{
        io::{Read, Write},
        num::NonZeroUsize,
        os::unix::net::UnixStream,
        sync::{Arc, Mutex},
        thread,
        time::{SystemTime, UNIX_EPOCH},
    };

    let socket_path = std::env::temp_dir().join(format!(
        "sophia-x-server-frontend-concurrent-clients-test-{}-{}.sock",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let config = XServerFrontendConfig::new(&socket_path, NamespaceId::from_raw(820))
        .unwrap()
        .with_max_concurrent_clients(NonZeroUsize::new(2).unwrap());
    let observations = Arc::new(Mutex::new(Vec::new()));
    let server_observations = observations.clone();
    let server = thread::spawn(move || {
        let mut frontend = XServerFrontend::bind(config).unwrap();
        let observer: Arc<X11CoreTraceObserver> = Arc::new(move |trace| {
            server_observations
                .lock()
                .unwrap()
                .push((trace.client.raw(), trace.major_opcode));
            Ok(())
        });
        frontend
            .serve_next_concurrently_traced(observer.clone())
            .unwrap();
        frontend.serve_next_concurrently_traced(observer).unwrap();
        assert_eq!(
            frontend.serve_next_concurrently().unwrap_err().to_string(),
            "Sophia X Server Frontend concurrent-client limit (2) reached"
        );
        frontend.wait_for_clients().unwrap();
        frontend.active_client_count()
    });

    wait_for_socket(&socket_path);
    let mut first = UnixStream::connect(&socket_path).unwrap();
    first
        .write_all(&setup_request(XByteOrder::LittleEndian, 11, 0, b"", b""))
        .unwrap();
    assert_eq!(
        read_setup_resource_id_base(&mut first, XByteOrder::LittleEndian),
        X_SETUP_DEFAULT_RESOURCE_ID_BASE
    );
    let first_window = 0x0020_0001;
    first
        .write_all(&create_window_request(
            XByteOrder::LittleEndian,
            first_window,
            0,
            0,
            160,
            90,
        ))
        .unwrap();
    assert_eq!(read_x_record(&mut first)[0], 22);

    let mut second = UnixStream::connect(&socket_path).unwrap();
    second
        .write_all(&setup_request(XByteOrder::LittleEndian, 11, 0, b"", b""))
        .unwrap();
    assert_eq!(
        read_setup_resource_id_base(&mut second, XByteOrder::LittleEndian),
        0x0040_0000
    );
    second
        .write_all(&resource_request(XByteOrder::LittleEndian, 8, first_window))
        .unwrap();
    let mut map_notify = [0; 32];
    second.read_exact(&mut map_notify).unwrap();
    assert_eq!(map_notify[0], 19);

    drop(first);
    drop(second);

    assert_eq!(server.join().unwrap(), 0);
    assert_eq!(
        observations.lock().unwrap().as_slice(),
        &[(1, 1), (2, 8), (1, 0)]
    );
    std::fs::remove_file(&socket_path).unwrap();
}

#[cfg(unix)]
#[test]
fn x_server_frontend_emits_surface_removal_when_a_client_disconnects() {
    use std::io::Write;
    use std::os::unix::net::UnixStream;
    use std::thread;
    use std::time::{SystemTime, UNIX_EPOCH};

    let socket_path = std::env::temp_dir().join(format!(
        "sophia-x-server-frontend-disconnect-cleanup-test-{}-{}.sock",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let config = XServerFrontendConfig::new(&socket_path, NamespaceId::from_raw(819)).unwrap();
    let server = thread::spawn(move || {
        let mut frontend = XServerFrontend::bind(config).unwrap();
        let mut removals = Vec::new();
        frontend
            .serve_next_traced(|trace| {
                if trace.request_detail.as_deref() == Some("DisconnectCleanup") {
                    removals.push((
                        trace.client.raw(),
                        trace
                            .result
                            .response
                            .as_ref()
                            .unwrap()
                            .removed_surfaces
                            .clone(),
                    ));
                }
                Ok(())
            })
            .unwrap();
        (removals, frontend.active_client_count())
    });

    wait_for_socket(&socket_path);
    let mut stream = UnixStream::connect(&socket_path).unwrap();
    stream
        .write_all(&setup_request(XByteOrder::LittleEndian, 11, 0, b"", b""))
        .unwrap();
    read_setup_success(&mut stream, XByteOrder::LittleEndian);
    stream
        .write_all(&create_window_request(
            XByteOrder::LittleEndian,
            0x0020_0001,
            0,
            0,
            160,
            90,
        ))
        .unwrap();
    let configure = read_x_record(&mut stream);
    assert_eq!(configure[0], 22);
    drop(stream);

    assert_eq!(
        server.join().unwrap(),
        (vec![(1, vec![SurfaceId::new(0x0020_0001, 1)])], 0)
    );
    std::fs::remove_file(&socket_path).unwrap();
}

#[cfg(unix)]
#[test]
fn x_server_frontend_rejects_create_window_outside_client_resource_range() {
    use std::io::Write;
    use std::os::unix::net::UnixStream;
    use std::thread;
    use std::time::{SystemTime, UNIX_EPOCH};

    let socket_path = std::env::temp_dir().join(format!(
        "sophia-x-server-frontend-resource-owner-test-{}-{}.sock",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let config = XServerFrontendConfig::new(&socket_path, NamespaceId::from_raw(817)).unwrap();
    let server = thread::spawn(move || {
        let mut frontend = XServerFrontend::bind(config).unwrap();
        frontend.serve_next().unwrap();
    });

    wait_for_socket(&socket_path);
    let mut stream = UnixStream::connect(&socket_path).unwrap();
    stream
        .write_all(&setup_request(XByteOrder::LittleEndian, 11, 0, b"", b""))
        .unwrap();
    assert_eq!(
        read_setup_resource_id_base(&mut stream, XByteOrder::LittleEndian),
        X_SETUP_DEFAULT_RESOURCE_ID_BASE
    );
    stream
        .write_all(&create_window_request(
            XByteOrder::LittleEndian,
            0x0040_0001,
            1,
            2,
            300,
            200,
        ))
        .unwrap();
    let error = read_x_record(&mut stream);
    assert_eq!(error[0], 0);
    assert_eq!(error[1], 14);

    drop(stream);
    server.join().unwrap();
    std::fs::remove_file(&socket_path).unwrap();
}

#[cfg(unix)]
#[test]
fn x11_setup_socket_smoke_completes_handshake() {
    use std::io::{Read, Write};
    use std::os::unix::net::UnixStream;
    use std::thread;
    use std::time::{SystemTime, UNIX_EPOCH};

    let socket_path = std::env::temp_dir().join(format!(
        "sophia-x11-setup-test-{}-{}.sock",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let server_path = socket_path.clone();
    let server = thread::spawn(move || run_x11_setup_socket_server_once(&server_path).unwrap());

    wait_for_socket(&socket_path);
    let mut stream = UnixStream::connect(&socket_path).unwrap();
    stream
        .write_all(&setup_request(
            XByteOrder::LittleEndian,
            11,
            0,
            b"MIT-MAGIC-COOKIE-1",
            b"0123456789abcdef",
        ))
        .unwrap();

    let mut prefix = [0; X_SETUP_REPLY_PREFIX_LEN];
    stream.read_exact(&mut prefix).unwrap();
    assert_eq!(prefix[0], 1);
    let body_len = usize::from(read_u16(XByteOrder::LittleEndian, &prefix[6..8])) * 4;
    let mut body = vec![0; body_len];
    stream.read_exact(&mut body).unwrap();

    assert_eq!(read_u32(XByteOrder::LittleEndian, &body[4..8]), 0x0020_0000);
    assert_eq!(
        read_u32(XByteOrder::LittleEndian, &body[8..12]),
        0x001f_ffff
    );
    let _ = std::fs::remove_file(&socket_path);
    server.join().unwrap();
}

#[cfg(unix)]
#[test]
fn x11_core_socket_smoke_round_trips_atom_property_and_window_events() {
    use std::io::Write;
    use std::os::unix::net::UnixStream;
    use std::thread;
    use std::time::{SystemTime, UNIX_EPOCH};

    let socket_path = std::env::temp_dir().join(format!(
        "sophia-x11-core-test-{}-{}.sock",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let server_path = socket_path.clone();
    let server = thread::spawn(move || {
        run_x11_core_socket_server_once(&server_path, NamespaceId::from_raw(47)).unwrap();
    });

    wait_for_socket(&socket_path);
    let mut stream = UnixStream::connect(&socket_path).unwrap();
    stream
        .write_all(&setup_request(XByteOrder::LittleEndian, 11, 0, b"", b""))
        .unwrap();
    read_setup_success(&mut stream, XByteOrder::LittleEndian);

    stream
        .write_all(&intern_atom_request(
            XByteOrder::LittleEndian,
            false,
            X_ATOM_NAME_NET_WM_NAME,
        ))
        .unwrap();
    let intern = read_x_record(&mut stream);
    assert_eq!(intern[0], 1);
    let net_wm_name = read_u32(XByteOrder::LittleEndian, &intern[8..12]);
    assert_ne!(net_wm_name, 0);

    stream
        .write_all(&intern_atom_request(
            XByteOrder::LittleEndian,
            false,
            X_ATOM_NAME_UTF8_STRING,
        ))
        .unwrap();
    let intern = read_x_record(&mut stream);
    assert_eq!(intern[0], 1);
    let utf8 = read_u32(XByteOrder::LittleEndian, &intern[8..12]);
    assert_ne!(utf8, 0);

    stream
        .write_all(&create_window_request(
            XByteOrder::LittleEndian,
            0x220201,
            1,
            2,
            300,
            200,
        ))
        .unwrap();
    let configure = read_x_record(&mut stream);
    assert_eq!(configure[0], 22);
    assert_eq!(
        read_u32(XByteOrder::LittleEndian, &configure[8..12]),
        0x220201
    );

    stream
        .write_all(&resource_request(XByteOrder::LittleEndian, 8, 0x220201))
        .unwrap();
    let map = read_x_record(&mut stream);
    assert_eq!(map[0], 19);
    assert_eq!(read_u32(XByteOrder::LittleEndian, &map[8..12]), 0x220201);
    let expose = read_x_record(&mut stream);
    assert_eq!(expose[0], 12);
    assert_eq!(read_u32(XByteOrder::LittleEndian, &expose[4..8]), 0x220201);

    stream
        .write_all(&change_property_request(
            XByteOrder::LittleEndian,
            XPropertyMode::Replace,
            0x220201,
            net_wm_name,
            utf8,
            8,
            b"Sophia Socket",
        ))
        .unwrap();
    let property = read_x_record(&mut stream);
    assert_eq!(property[0], 28);
    assert_eq!(
        read_u32(XByteOrder::LittleEndian, &property[8..12]),
        net_wm_name
    );

    stream
        .write_all(&get_property_request(
            XByteOrder::LittleEndian,
            false,
            0x220201,
            net_wm_name,
            X_PROPERTY_ANY_TYPE,
            0,
            64,
        ))
        .unwrap();
    let property = read_x_reply(&mut stream, XByteOrder::LittleEndian);
    assert_eq!(property[0], 1);
    assert_eq!(property[1], 8);
    assert_eq!(read_u32(XByteOrder::LittleEndian, &property[8..12]), utf8);
    assert_eq!(&property[32..45], b"Sophia Socket");

    drop(stream);
    let _ = std::fs::remove_file(&socket_path);
    server.join().unwrap();
}

#[cfg(unix)]
#[test]
fn x11_core_listener_reclaims_disconnected_client_window_before_next_client() {
    use std::io::Write;
    use std::os::unix::net::UnixStream;
    use std::thread;
    use std::time::{SystemTime, UNIX_EPOCH};

    let socket_path = std::env::temp_dir().join(format!(
        "sophia-x11-persistent-core-test-{}-{}.sock",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let server_path = socket_path.clone();
    let server = thread::spawn(move || {
        let listener = bind_x11_core_socket_server(&server_path).unwrap();
        let mut state = X11CoreSocketServerState::new();
        serve_x11_core_socket_listener_once(&listener, NamespaceId::from_raw(52), &mut state)
            .unwrap();
        serve_x11_core_socket_listener_once(&listener, NamespaceId::from_raw(52), &mut state)
            .unwrap();
    });

    wait_for_socket(&socket_path);
    let mut first = UnixStream::connect(&socket_path).unwrap();
    first
        .write_all(&setup_request(XByteOrder::LittleEndian, 11, 0, b"", b""))
        .unwrap();
    read_setup_success(&mut first, XByteOrder::LittleEndian);
    first
        .write_all(&create_window_request(
            XByteOrder::LittleEndian,
            0x220701,
            1,
            2,
            300,
            200,
        ))
        .unwrap();
    let configure = read_x_record(&mut first);
    assert_eq!(configure[0], 22);
    drop(first);

    let mut second = UnixStream::connect(&socket_path).unwrap();
    second
        .write_all(&setup_request(XByteOrder::LittleEndian, 11, 0, b"", b""))
        .unwrap();
    read_setup_success(&mut second, XByteOrder::LittleEndian);
    second
        .write_all(&resource_request(XByteOrder::LittleEndian, 8, 0x220701))
        .unwrap();
    let error = read_x_record(&mut second);
    assert_eq!(error[0], 0);
    assert_eq!(error[1], 3, "the released window must be BadWindow");
    assert_eq!(read_u32(XByteOrder::LittleEndian, &error[4..8]), 0x220701);

    drop(second);
    server.join().unwrap();
    let _ = std::fs::remove_file(&socket_path);
}

#[cfg(unix)]
#[test]
fn x11_core_socket_observer_sees_poly_fill_rectangle_transaction() {
    use std::io::Write;
    use std::os::unix::net::UnixStream;
    use std::thread;
    use std::time::{SystemTime, UNIX_EPOCH};

    let socket_path = std::env::temp_dir().join(format!(
        "sophia-x11-core-draw-test-{}-{}.sock",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let server_path = socket_path.clone();
    let server = thread::spawn(move || {
        let mut transactions = 0usize;
        run_x11_core_socket_server_once_observed(
            &server_path,
            NamespaceId::from_raw(48),
            |result| {
                if let Some(response) = &result.response {
                    transactions = transactions.saturating_add(response.transactions.len());
                }
            },
        )
        .unwrap();
        transactions
    });

    wait_for_socket(&socket_path);
    let mut stream = UnixStream::connect(&socket_path).unwrap();
    stream
        .write_all(&setup_request(XByteOrder::LittleEndian, 11, 0, b"", b""))
        .unwrap();
    read_setup_success(&mut stream, XByteOrder::LittleEndian);

    stream
        .write_all(&create_window_request(
            XByteOrder::LittleEndian,
            0x220301,
            1,
            2,
            300,
            200,
        ))
        .unwrap();
    let configure = read_x_record(&mut stream);
    assert_eq!(configure[0], 22);

    stream
        .write_all(&create_gc_request(
            XByteOrder::LittleEndian,
            0x220302,
            0x220301,
        ))
        .unwrap();
    stream
        .write_all(&poly_fill_rectangle_request(
            XByteOrder::LittleEndian,
            0x220301,
            0x220302,
            &[(5, 6, 40, 30)],
        ))
        .unwrap();

    drop(stream);
    let _ = std::fs::remove_file(&socket_path);
    assert_eq!(server.join().unwrap(), 1);
}

#[cfg(unix)]
#[test]
fn x11_core_socket_observer_sees_put_image_transaction() {
    use std::io::Write;
    use std::os::unix::net::UnixStream;
    use std::thread;
    use std::time::{SystemTime, UNIX_EPOCH};

    let socket_path = std::env::temp_dir().join(format!(
        "sophia-x11-put-image-test-{}-{}.sock",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let server_path = socket_path.clone();
    let server = thread::spawn(move || {
        let mut transactions = 0usize;
        run_x11_core_socket_server_once_observed(
            &server_path,
            NamespaceId::from_raw(49),
            |result| {
                if let Some(response) = &result.response {
                    transactions = transactions.saturating_add(response.transactions.len());
                }
            },
        )
        .unwrap();
        transactions
    });

    wait_for_socket(&socket_path);
    let mut stream = UnixStream::connect(&socket_path).unwrap();
    stream
        .write_all(&setup_request(XByteOrder::LittleEndian, 11, 0, b"", b""))
        .unwrap();
    read_setup_success(&mut stream, XByteOrder::LittleEndian);

    stream
        .write_all(&create_window_request(
            XByteOrder::LittleEndian,
            0x220401,
            1,
            2,
            300,
            200,
        ))
        .unwrap();
    let configure = read_x_record(&mut stream);
    assert_eq!(configure[0], 22);

    stream
        .write_all(&create_gc_request(
            XByteOrder::LittleEndian,
            0x220402,
            0x220401,
        ))
        .unwrap();
    stream
        .write_all(&put_image_request(
            XByteOrder::LittleEndian,
            0x220401,
            0x220402,
            8,
            4,
            3,
            5,
            &[0xaa; 128],
        ))
        .unwrap();

    drop(stream);
    let _ = std::fs::remove_file(&socket_path);
    assert_eq!(server.join().unwrap(), 1);
}

#[cfg(unix)]
#[test]
fn x11_core_socket_observer_sees_sophia_present_transaction() {
    use std::io::Write;
    use std::os::unix::net::UnixStream;
    use std::thread;
    use std::time::{SystemTime, UNIX_EPOCH};

    let socket_path = std::env::temp_dir().join(format!(
        "sophia-x11-present-test-{}-{}.sock",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let server_path = socket_path.clone();
    let server = thread::spawn(move || {
        let mut transactions = 0usize;
        run_x11_core_socket_server_once_observed(
            &server_path,
            NamespaceId::from_raw(50),
            |result| {
                if let Some(response) = &result.response {
                    transactions = transactions.saturating_add(response.transactions.len());
                }
            },
        )
        .unwrap();
        transactions
    });

    wait_for_socket(&socket_path);
    let mut stream = UnixStream::connect(&socket_path).unwrap();
    stream
        .write_all(&setup_request(XByteOrder::LittleEndian, 11, 0, b"", b""))
        .unwrap();
    read_setup_success(&mut stream, XByteOrder::LittleEndian);

    stream
        .write_all(&query_extension_request(
            XByteOrder::LittleEndian,
            X_SOPHIA_PRESENT_EXTENSION_NAME,
        ))
        .unwrap();
    let query = read_x_record(&mut stream);
    assert_eq!(query[8], 1);
    assert_eq!(query[9], X_SOPHIA_PRESENT_MAJOR_OPCODE);

    stream
        .write_all(&create_window_request(
            XByteOrder::LittleEndian,
            0x220501,
            1,
            2,
            300,
            200,
        ))
        .unwrap();
    let configure = read_x_record(&mut stream);
    assert_eq!(configure[0], 22);

    stream
        .write_all(&sophia_present_pixmap_request(
            XByteOrder::LittleEndian,
            0x220501,
            0x990,
            (3, 5, 32, 24),
            1,
            250,
        ))
        .unwrap();

    drop(stream);
    let _ = std::fs::remove_file(&socket_path);
    assert_eq!(server.join().unwrap(), 1);
}

#[cfg(unix)]
#[test]
fn x11_core_socket_channel_sees_sophia_present_transaction_batch() {
    use std::io::Write;
    use std::os::unix::net::UnixStream;
    use std::thread;
    use std::time::{SystemTime, UNIX_EPOCH};

    let socket_path = std::env::temp_dir().join(format!(
        "sophia-x11-present-channel-test-{}-{}.sock",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let server_path = socket_path.clone();
    let (sender, receiver) =
        std::sync::mpsc::sync_channel(X_AUTHORITY_OBSERVED_TRANSACTION_CHANNEL_CAPACITY);
    let server = thread::spawn(move || {
        run_x11_core_socket_server_once_channel(&server_path, NamespaceId::from_raw(51), sender)
            .unwrap();
    });

    wait_for_socket(&socket_path);
    let mut stream = UnixStream::connect(&socket_path).unwrap();
    stream
        .write_all(&setup_request(XByteOrder::LittleEndian, 11, 0, b"", b""))
        .unwrap();
    read_setup_success(&mut stream, XByteOrder::LittleEndian);

    stream
        .write_all(&create_window_request(
            XByteOrder::LittleEndian,
            0x220601,
            1,
            2,
            300,
            200,
        ))
        .unwrap();
    let configure = read_x_record(&mut stream);
    assert_eq!(configure[0], 22);

    stream
        .write_all(&sophia_present_pixmap_request(
            XByteOrder::LittleEndian,
            0x220601,
            0x991,
            (3, 5, 32, 24),
            1,
            250,
        ))
        .unwrap();

    drop(stream);
    let _ = std::fs::remove_file(&socket_path);
    server.join().unwrap();
    let batch = receiver.try_recv().unwrap();
    assert_eq!(batch.client.map(XServerFrontendClientId::raw), Some(1));
    assert_eq!(batch.transaction, TransactionId::from_raw(2));
    assert_eq!(batch.transactions.len(), 1);
    assert_eq!(
        batch.transactions[0].transaction,
        TransactionId::from_raw(2)
    );
    let surface = batch.transactions[0].surface;
    let mut routes = XAuthorityClientSurfaceRoutes::default();
    routes.observe(&batch);
    assert_eq!(
        routes
            .client_for_surface(surface)
            .map(XServerFrontendClientId::raw),
        Some(1)
    );
    routes.observe(&XAuthorityObservedTransactionBatch {
        client: None,
        transaction: TransactionId::from_raw(3),
        transactions: Vec::new(),
        removed_surfaces: vec![surface],
        cpu_buffer_updates: Vec::new(),
        dma_buf_registrations: Vec::new(),
        fence_registrations: Vec::new(),
        present_submissions: Vec::new(),
        released_dma_bufs: Vec::new(),
        released_fences: Vec::new(),
        protocol_errors: Vec::new(),
    });
    assert!(routes.is_empty());
}

#[cfg(unix)]
#[test]
fn routed_service_confines_input_and_control_to_two_workers_and_drains() {
    use std::io::{Read, Write};
    use std::num::NonZeroUsize;
    use std::os::unix::net::UnixStream;
    use std::sync::Arc;
    use std::thread;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    let socket_path = std::env::temp_dir().join(format!(
        "sophia-x11-routed-worker-test-{}-{}.sock",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let server_path = socket_path.clone();
    let (transaction_sender, transaction_receiver) =
        std::sync::mpsc::sync_channel(X_AUTHORITY_OBSERVED_TRANSACTION_CHANNEL_CAPACITY);
    let (acknowledgement_sender, acknowledgement_receiver) = std::sync::mpsc::sync_channel(4);
    let broker = XServerFrontendRouteBroker::with_control_ack_sender(
        NonZeroUsize::new(4).unwrap(),
        acknowledgement_sender,
    );
    let input_sender = broker.routed_input_sender();
    let control_sender = broker.control_sender();
    let (service_command_sender, service_command_receiver) = std::sync::mpsc::sync_channel(1);
    let first_namespace = NamespaceContext::new(
        NamespaceId::from_raw(852),
        NamespaceProfile::Confined,
        NamespaceCapabilities::NONE,
    )
    .unwrap();
    let second_namespace = NamespaceContext::new(
        NamespaceId::from_raw(853),
        NamespaceProfile::Confined,
        NamespaceCapabilities::NONE,
    )
    .unwrap();
    let policy = Arc::new(SequencedXAdmissionPolicy {
        namespaces: [first_namespace, second_namespace],
        next_client: std::sync::atomic::AtomicU64::new(0),
        revoked: std::sync::Mutex::new(Vec::new()),
    });
    let config = XServerFrontendConfig::new_with_namespace_context(&server_path, first_namespace)
        .unwrap()
        .with_admission_policy(policy.clone())
        .with_max_concurrent_clients(NonZeroUsize::new(2).unwrap());
    let server = thread::spawn(move || {
        run_x_server_frontend_routed_until_stopped(
            config,
            transaction_sender,
            broker,
            service_command_receiver,
        )
        .unwrap();
    });

    wait_for_socket(&socket_path);
    let mut first = UnixStream::connect(&socket_path).unwrap();
    first
        .write_all(&setup_request(XByteOrder::LittleEndian, 11, 0, b"", b""))
        .unwrap();
    read_setup_success(&mut first, XByteOrder::LittleEndian);
    first
        .write_all(&create_window_request(
            XByteOrder::LittleEndian,
            0x0020_0701,
            1,
            2,
            300,
            200,
        ))
        .unwrap();
    assert_eq!(read_x_record(&mut first)[0], 22);
    first
        .write_all(&change_window_event_mask_request(
            XByteOrder::LittleEndian,
            0x0020_0701,
            0b11,
        ))
        .unwrap();
    first
        .write_all(&sophia_present_pixmap_request(
            XByteOrder::LittleEndian,
            0x0020_0701,
            0x992,
            (0, 0, 16, 16),
            1,
            1,
        ))
        .unwrap();

    let mut second = UnixStream::connect(&socket_path).unwrap();
    second
        .write_all(&setup_request(XByteOrder::LittleEndian, 11, 0, b"", b""))
        .unwrap();
    read_setup_success(&mut second, XByteOrder::LittleEndian);
    second
        .write_all(&create_window_request(
            XByteOrder::LittleEndian,
            0x0040_0702,
            3,
            4,
            300,
            200,
        ))
        .unwrap();
    assert_eq!(read_x_record(&mut second)[0], 22);
    second
        .write_all(&change_window_event_mask_request(
            XByteOrder::LittleEndian,
            0x0020_0701,
            0b11,
        ))
        .unwrap();
    let mut error = [0; 32];
    second.read_exact(&mut error).unwrap();
    assert_eq!(error[0], 0);
    assert_eq!(error[1], XErrorCode::BadAccess.wire_code());
    second
        .write_all(&sophia_present_pixmap_request(
            XByteOrder::LittleEndian,
            0x0040_0702,
            0x993,
            (0, 0, 16, 16),
            1,
            1,
        ))
        .unwrap();

    let mut routes = Vec::new();
    let mut observed_protocol_error = false;
    while routes.len() < 2 || !observed_protocol_error {
        let batch = transaction_receiver
            .recv_timeout(Duration::from_secs(1))
            .unwrap();
        if batch.transactions.is_empty() {
            assert_eq!(batch.protocol_errors.len(), 1);
            assert_eq!(
                batch.protocol_errors[0].code,
                XErrorCode::BadAccess.wire_code()
            );
            observed_protocol_error = true;
            continue;
        }
        if routes.len() < 2 {
            routes.push((
                batch
                    .client
                    .expect("routed worker must identify its client"),
                batch.transactions[0].surface,
            ));
        }
    }
    assert!(observed_protocol_error);
    routes.sort_by_key(|(client, _)| client.raw());
    assert_ne!(routes[0].0, routes[1].0);
    for (index, (_, surface)) in routes.iter().copied().enumerate() {
        input_sender
            .send(XAuthorityRoutedInput {
                request: RoutedInputRequest {
                    serial: 20 + index as u64,
                    seat: SeatId::from_raw(1),
                    device: DeviceId::from_raw(1),
                    time_msec: 10 + index as u64,
                    target_surface: surface,
                    global_position: Point::default(),
                    local_position: Point::default(),
                    kind: InputEventKind::Key {
                        keycode: 30 + index as u32,
                        pressed: true,
                    },
                },
                delivery: None,
            })
            .unwrap();
    }
    assert_eq!(read_x_record(&mut first)[0], 9);
    let first_key = read_x_record(&mut first);
    assert_eq!(first_key[0], 2);
    assert_eq!(first_key[1], 38);
    assert_eq!(
        read_u32(XByteOrder::LittleEndian, &first_key[12..16]),
        0x0020_0701
    );
    assert_eq!(read_x_record(&mut second)[0], 9);
    let second_key = read_x_record(&mut second);
    assert_eq!(second_key[0], 2);
    assert_eq!(second_key[1], 39);
    assert_eq!(
        read_u32(XByteOrder::LittleEndian, &second_key[12..16]),
        0x0040_0702
    );
    for (index, (client, surface)) in routes.iter().copied().enumerate() {
        control_sender
            .send(XAuthorityClientControlCommand {
                client,
                command: XAuthorityControlCommand::ConfigureSurface {
                    transaction: TransactionId::from_raw(88 + index as u64),
                    surface,
                    size: Size {
                        width: 301 + index as i32,
                        height: 201 + index as i32,
                    },
                },
            })
            .unwrap();
    }
    let mut acknowledgements = Vec::new();
    for _ in 0..2 {
        acknowledgements.push(
            acknowledgement_receiver
                .recv_timeout(Duration::from_secs(1))
                .unwrap(),
        );
    }
    for (index, (client, surface)) in routes.iter().copied().enumerate() {
        assert!(acknowledgements.contains(&XAuthorityClientControlAck {
            client,
            acknowledgement: XAuthorityControlAck {
                transaction: TransactionId::from_raw(88 + index as u64),
                surface,
                outcome: XAuthorityControlOutcome::Delivered,
            },
        }));
    }
    assert_eq!(read_x_record(&mut first)[0], 22);
    assert_eq!(read_x_record(&mut first)[0], 12);
    assert_eq!(read_x_record(&mut second)[0], 22);
    assert_eq!(read_x_record(&mut second)[0], 12);

    drop(first);
    drop(second);
    service_command_sender
        .send(XServerFrontendServiceCommand::StopAccepting)
        .unwrap();
    drop(service_command_sender);
    drop(input_sender);
    drop(control_sender);
    server.join().unwrap();
    let revoked = policy.revoked.lock().unwrap();
    assert_eq!(revoked.len(), 2);
    assert_ne!(revoked[0].namespace.id, revoked[1].namespace.id);
    std::fs::remove_file(&socket_path).unwrap();
}

#[cfg(unix)]
#[test]
fn routed_service_applies_topology_update_and_notifies_randr_subscriber() {
    use std::io::Write;
    use std::num::NonZeroUsize;
    use std::os::unix::net::UnixStream;
    use std::thread;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    let socket_path = std::env::temp_dir().join(format!(
        "sophia-x11-randr-update-test-{}-{}.sock",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let server_path = socket_path.clone();
    let (transaction_sender, _transaction_receiver) =
        std::sync::mpsc::sync_channel(X_AUTHORITY_OBSERVED_TRANSACTION_CHANNEL_CAPACITY);
    let broker = XServerFrontendRouteBroker::new(NonZeroUsize::new(4).unwrap());
    let (service_sender, service_receiver) = std::sync::mpsc::sync_channel(2);
    let config = XServerFrontendConfig::new(&server_path, NamespaceId::from_raw(854)).unwrap();
    let server = thread::spawn(move || {
        run_x_server_frontend_routed_until_stopped(
            config,
            transaction_sender,
            broker,
            service_receiver,
        )
        .unwrap();
    });

    wait_for_socket(&socket_path);
    let mut stream = UnixStream::connect(&socket_path).unwrap();
    stream
        .write_all(&setup_request(XByteOrder::LittleEndian, 11, 0, b"", b""))
        .unwrap();
    read_setup_success(&mut stream, XByteOrder::LittleEndian);
    stream
        .write_all(&randr_select_input_request(
            XByteOrder::LittleEndian,
            X_SETUP_DEFAULT_ROOT,
            0x47,
        ))
        .unwrap();
    stream
        .write_all(&randr_window_request(
            XByteOrder::LittleEndian,
            X_RANDR_GET_SCREEN_RESOURCES_CURRENT_MINOR_OPCODE,
            X_SETUP_DEFAULT_ROOT,
        ))
        .unwrap();
    assert_eq!(read_x_reply(&mut stream, XByteOrder::LittleEndian)[0], 1);

    let snapshot = OutputTopologySnapshot {
        generation: 2,
        primary: OutputId::from_raw(9),
        outputs: vec![OutputTopologyEntry {
            output: OutputId::from_raw(9),
            logical: Rect {
                x: 0,
                y: 0,
                width: 1600,
                height: 900,
            },
            pixel_size: Size {
                width: 1600,
                height: 900,
            },
            scale: 1,
            refresh_millihz: 60_000,
        }],
    };
    let (ack_sender, ack_receiver) = std::sync::mpsc::sync_channel(1);
    service_sender
        .send(XServerFrontendServiceCommand::UpdateOutputTopology {
            snapshot,
            acknowledgement: ack_sender,
        })
        .unwrap();
    assert_eq!(
        ack_receiver.recv_timeout(Duration::from_secs(1)).unwrap(),
        XAuthorityOutputUpdateOutcome::Applied {
            generation: 2,
            notifications: 4,
        }
    );
    let event = read_x_record(&mut stream);
    assert_eq!(event[0], X_RANDR_FIRST_EVENT, "event={event:?}");
    assert_eq!(read_u32(XByteOrder::LittleEndian, &event[8..12]), 2);
    assert_eq!(read_u16(XByteOrder::LittleEndian, &event[24..26]), 1600);
    assert_eq!(read_u16(XByteOrder::LittleEndian, &event[26..28]), 900);
    let crtc = read_x_record(&mut stream);
    assert_eq!(crtc[0], X_RANDR_FIRST_EVENT + 1);
    assert_eq!(crtc[1], 0);
    assert_eq!(
        read_u32(XByteOrder::LittleEndian, &crtc[12..16]),
        0x1000_0009
    );
    assert_eq!(read_u16(XByteOrder::LittleEndian, &crtc[28..30]), 1600);
    assert_eq!(read_u16(XByteOrder::LittleEndian, &crtc[30..32]), 900);
    let output = read_x_record(&mut stream);
    assert_eq!(output[0], X_RANDR_FIRST_EVENT + 1);
    assert_eq!(output[1], 1);
    assert_eq!(
        read_u32(XByteOrder::LittleEndian, &output[16..20]),
        0x2000_0009
    );
    let resources = read_x_record(&mut stream);
    assert_eq!(resources[0], X_RANDR_FIRST_EVENT + 1);
    assert_eq!(resources[1], 5);
    assert_eq!(read_u32(XByteOrder::LittleEndian, &resources[4..8]), 2);

    drop(stream);
    service_sender
        .send(XServerFrontendServiceCommand::StopAccepting)
        .unwrap();
    drop(service_sender);
    server.join().unwrap();
    std::fs::remove_file(&socket_path).unwrap();
}

#[cfg(unix)]
#[test]
fn routed_service_revokes_one_live_admission_without_disrupting_its_classic_peer() {
    use std::io::{Read, Write};
    use std::num::NonZeroUsize;
    use std::os::unix::net::UnixStream;
    use std::sync::Arc;
    use std::thread;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    let socket_path = std::env::temp_dir().join(format!(
        "sophia-x11-supervisor-revocation-test-{}-{}.sock",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let namespace = NamespaceContext::new(
        NamespaceId::from_raw(854),
        NamespaceProfile::ClassicShared,
        NamespaceCapabilities::NONE,
    )
    .unwrap();
    let policy = Arc::new(TestXAdmissionPolicy::new(namespace, false));
    let config = XServerFrontendConfig::new_with_namespace_context(&socket_path, namespace)
        .unwrap()
        .with_admission_policy(policy.clone())
        .with_max_concurrent_clients(NonZeroUsize::new(2).unwrap());
    let (transaction_sender, transaction_receiver) =
        std::sync::mpsc::sync_channel(X_AUTHORITY_OBSERVED_TRANSACTION_CHANNEL_CAPACITY);
    let broker = XServerFrontendRouteBroker::new(NonZeroUsize::new(4).unwrap());
    let (service_command_sender, service_command_receiver) = std::sync::mpsc::sync_channel(2);
    let server = thread::spawn(move || {
        run_x_server_frontend_routed_until_stopped(
            config,
            transaction_sender,
            broker,
            service_command_receiver,
        )
        .unwrap();
    });

    wait_for_socket(&socket_path);
    let mut first = UnixStream::connect(&socket_path).unwrap();
    first
        .write_all(&setup_request(XByteOrder::LittleEndian, 11, 0, b"", b""))
        .unwrap();
    read_setup_success(&mut first, XByteOrder::LittleEndian);
    first
        .write_all(&create_window_request(
            XByteOrder::LittleEndian,
            0x0020_0801,
            1,
            2,
            160,
            90,
        ))
        .unwrap();
    assert_eq!(read_x_record(&mut first)[0], 22);
    first
        .write_all(&sophia_present_pixmap_request(
            XByteOrder::LittleEndian,
            0x0020_0801,
            0x994,
            (0, 0, 16, 16),
            1,
            1,
        ))
        .unwrap();

    let mut second = UnixStream::connect(&socket_path).unwrap();
    second
        .write_all(&setup_request(XByteOrder::LittleEndian, 11, 0, b"", b""))
        .unwrap();
    read_setup_success(&mut second, XByteOrder::LittleEndian);
    second
        .write_all(&create_window_request(
            XByteOrder::LittleEndian,
            0x0040_0802,
            3,
            4,
            160,
            90,
        ))
        .unwrap();
    assert_eq!(read_x_record(&mut second)[0], 22);
    second
        .write_all(&sophia_present_pixmap_request(
            XByteOrder::LittleEndian,
            0x0040_0802,
            0x995,
            (0, 0, 16, 16),
            1,
            1,
        ))
        .unwrap();

    let mut initial_batches = Vec::new();
    for _ in 0..2 {
        initial_batches.push(
            transaction_receiver
                .recv_timeout(Duration::from_secs(1))
                .unwrap(),
        );
    }
    let first_client = initial_batches
        .iter()
        .find_map(|batch| {
            (batch.client.map(XServerFrontendClientId::raw) == Some(1))
                .then(|| batch.client.unwrap())
        })
        .unwrap();

    service_command_sender
        .send(XServerFrontendServiceCommand::RevokeAdmission {
            admission: ClientAdmissionId::from_raw(1),
        })
        .unwrap();
    first
        .set_read_timeout(Some(Duration::from_secs(1)))
        .unwrap();
    let mut disconnected = [0u8; 1];
    match first.read(&mut disconnected) {
        Ok(0) => {}
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::BrokenPipe
                    | std::io::ErrorKind::ConnectionReset
                    | std::io::ErrorKind::UnexpectedEof
            ) => {}
        outcome => panic!("revoked X11 client remained connected: {outcome:?}"),
    }

    let cleanup = transaction_receiver
        .recv_timeout(Duration::from_secs(1))
        .unwrap();
    assert_eq!(cleanup.client, Some(first_client));
    assert_eq!(cleanup.removed_surfaces.len(), 1);

    second
        .write_all(&resource_request(XByteOrder::LittleEndian, 8, 0x0020_0801))
        .unwrap();
    let mut error = [0; 32];
    second.read_exact(&mut error).unwrap();
    assert_eq!(error[0], 0);
    assert_eq!(error[1], XErrorCode::BadWindow.wire_code());
    second
        .write_all(&create_window_request(
            XByteOrder::LittleEndian,
            0x0040_0803,
            5,
            6,
            80,
            45,
        ))
        .unwrap();
    assert_eq!(read_x_record(&mut second)[0], 22);

    drop(first);
    drop(second);
    service_command_sender
        .send(XServerFrontendServiceCommand::StopAccepting)
        .unwrap();
    drop(service_command_sender);
    server.join().unwrap();
    let revoked = policy.revoked.lock().unwrap();
    assert_eq!(revoked.len(), 2);
    assert!(
        revoked
            .iter()
            .any(|context| context.client_id == ClientAdmissionId::from_raw(1))
    );
    assert!(
        revoked
            .iter()
            .any(|context| context.client_id == ClientAdmissionId::from_raw(2))
    );
    std::fs::remove_file(&socket_path).unwrap();
}

#[cfg(unix)]
#[test]
fn routed_service_retains_revocation_requested_before_admission_attaches() {
    use std::io::{Read, Write};
    use std::num::NonZeroUsize;
    use std::os::unix::net::UnixStream;
    use std::sync::Arc;
    use std::thread;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    let socket_path = std::env::temp_dir().join(format!(
        "sophia-x11-early-revocation-test-{}-{}.sock",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let namespace = NamespaceContext::new(
        NamespaceId::from_raw(855),
        NamespaceProfile::ClassicShared,
        NamespaceCapabilities::NONE,
    )
    .unwrap();
    let policy = Arc::new(TestXAdmissionPolicy::new(namespace, false));
    let config = XServerFrontendConfig::new_with_namespace_context(&socket_path, namespace)
        .unwrap()
        .with_admission_policy(policy.clone());
    let (transaction_sender, _transaction_receiver) =
        std::sync::mpsc::sync_channel(X_AUTHORITY_OBSERVED_TRANSACTION_CHANNEL_CAPACITY);
    let broker = XServerFrontendRouteBroker::new(NonZeroUsize::new(2).unwrap());
    let (service_command_sender, service_command_receiver) = std::sync::mpsc::sync_channel(2);
    let server = thread::spawn(move || {
        run_x_server_frontend_routed_until_stopped(
            config,
            transaction_sender,
            broker,
            service_command_receiver,
        )
        .unwrap();
    });

    wait_for_socket(&socket_path);
    service_command_sender
        .send(XServerFrontendServiceCommand::RevokeAdmission {
            admission: ClientAdmissionId::from_raw(1),
        })
        .unwrap();
    let mut client = UnixStream::connect(&socket_path).unwrap();
    client
        .write_all(&setup_request(XByteOrder::LittleEndian, 11, 0, b"", b""))
        .unwrap();
    read_setup_success(&mut client, XByteOrder::LittleEndian);
    client
        .set_read_timeout(Some(Duration::from_secs(1)))
        .unwrap();
    let mut disconnected = [0u8; 1];
    match client.read(&mut disconnected) {
        Ok(0) => {}
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::BrokenPipe
                    | std::io::ErrorKind::ConnectionReset
                    | std::io::ErrorKind::UnexpectedEof
            ) => {}
        outcome => panic!("early-revoked X11 client remained connected: {outcome:?}"),
    }

    drop(client);
    service_command_sender
        .send(XServerFrontendServiceCommand::StopAccepting)
        .unwrap();
    drop(service_command_sender);
    server.join().unwrap();
    let revoked = policy.revoked.lock().unwrap();
    assert_eq!(revoked.len(), 1);
    assert_eq!(revoked[0].client_id, ClientAdmissionId::from_raw(1));
    std::fs::remove_file(&socket_path).unwrap();
}

fn present_dispatch_result(transaction: TransactionId) -> XDispatchResult {
    let namespace = NamespaceId::from_raw(45);
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let window = XResourceId::new(0x220530, 1);
    runtime.apply(XAuthorityRequestPacket {
        transaction: TransactionId::from_raw(1),
        namespace,
        kind: XAuthorityRequestKind::CreateWindow {
            window,
            surface: SurfaceId::new(40, 1),
            geometry: Rect {
                x: 0,
                y: 0,
                width: 300,
                height: 200,
            },
            constraints: SurfaceConstraints {
                min_size: None,
                max_size: None,
            },
            generation: 1,
        },
    });

    dispatch_x11_wire_request(
        dispatch_context(
            namespace,
            u16::try_from(transaction.raw()).unwrap_or(u16::MAX),
            XByteOrder::LittleEndian,
            X_SOPHIA_PRESENT_MAJOR_OPCODE,
        ),
        XWireRequest::Authority(XAuthorityRequestPacket {
            transaction,
            namespace,
            kind: XAuthorityRequestKind::PresentPixmap {
                window,
                pixmap: 0x900,
                damage: Region::single(Rect {
                    x: 4,
                    y: 5,
                    width: 64,
                    height: 48,
                }),
                previous_committed_generation: 1,
                timeout_msec: 250,
            },
        }),
        &mut runtime,
        &mut atoms,
        &mut properties,
    )
}

fn context(namespace: NamespaceId, transaction: u64, byte_order: XByteOrder) -> XWireClientContext {
    XWireClientContext {
        byte_order,
        namespace,
        transaction: TransactionId::from_raw(transaction),
        resource_id_range: None,
    }
}

fn dispatch_context(
    namespace: NamespaceId,
    sequence: u16,
    byte_order: XByteOrder,
    major_opcode: u8,
) -> XDispatchContext {
    XDispatchContext {
        byte_order,
        namespace,
        sequence,
        major_opcode,
        client_id: 1,
    }
}

fn setup_request(
    byte_order: XByteOrder,
    major: u16,
    minor: u16,
    auth_name: &[u8],
    auth_data: &[u8],
) -> Vec<u8> {
    let mut out = Vec::new();
    out.push(byte_order.marker());
    out.push(0);
    push_u16(&mut out, byte_order, major);
    push_u16(&mut out, byte_order, minor);
    push_u16(&mut out, byte_order, auth_name.len() as u16);
    push_u16(&mut out, byte_order, auth_data.len() as u16);
    push_u16(&mut out, byte_order, 0);
    out.extend_from_slice(auth_name);
    pad_to_four(&mut out);
    out.extend_from_slice(auth_data);
    pad_to_four(&mut out);
    out
}

fn create_window_request(
    byte_order: XByteOrder,
    window: u32,
    x: i16,
    y: i16,
    width: u16,
    height: u16,
) -> Vec<u8> {
    let mut out = vec![1, 24];
    push_u16(&mut out, byte_order, 8);
    push_u32(&mut out, byte_order, window);
    push_u32(&mut out, byte_order, 0x20);
    push_i16(&mut out, byte_order, x);
    push_i16(&mut out, byte_order, y);
    push_u16(&mut out, byte_order, width);
    push_u16(&mut out, byte_order, height);
    push_u16(&mut out, byte_order, 0);
    push_u16(&mut out, byte_order, 1);
    push_u32(&mut out, byte_order, 0);
    push_u32(&mut out, byte_order, 0);
    out
}

#[allow(clippy::too_many_arguments)]
fn create_window_background_request(
    byte_order: XByteOrder,
    window: u32,
    x: i16,
    y: i16,
    width: u16,
    height: u16,
    background_pixel: u32,
) -> Vec<u8> {
    let mut out = create_window_request(byte_order, window, x, y, width, height);
    out[2..4].copy_from_slice(&match byte_order {
        XByteOrder::LittleEndian => 9u16.to_le_bytes(),
        XByteOrder::BigEndian => 9u16.to_be_bytes(),
    });
    out[28..32].copy_from_slice(&match byte_order {
        XByteOrder::LittleEndian => 2u32.to_le_bytes(),
        XByteOrder::BigEndian => 2u32.to_be_bytes(),
    });
    push_u32(&mut out, byte_order, background_pixel);
    out
}

fn resource_request(byte_order: XByteOrder, opcode: u8, id: u32) -> Vec<u8> {
    let mut out = vec![opcode, 0];
    push_u16(&mut out, byte_order, 2);
    push_u32(&mut out, byte_order, id);
    out
}

fn translate_coordinates_request(
    byte_order: XByteOrder,
    source: u32,
    destination: u32,
    src_x: i16,
    src_y: i16,
) -> Vec<u8> {
    let mut out = vec![40, 0];
    push_u16(&mut out, byte_order, 4);
    push_u32(&mut out, byte_order, source);
    push_u32(&mut out, byte_order, destination);
    push_i16(&mut out, byte_order, src_x);
    push_i16(&mut out, byte_order, src_y);
    out
}

fn intern_atom_request(byte_order: XByteOrder, only_if_exists: bool, name: &str) -> Vec<u8> {
    let mut out = vec![16, u8::from(only_if_exists)];
    let len_units = (8 + padded_len_for_test(name.len())) / 4;
    push_u16(&mut out, byte_order, len_units as u16);
    push_u16(&mut out, byte_order, name.len() as u16);
    push_u16(&mut out, byte_order, 0);
    out.extend_from_slice(name.as_bytes());
    pad_to_four(&mut out);
    out
}

fn get_atom_name_request(byte_order: XByteOrder, atom: u32) -> Vec<u8> {
    let mut out = vec![17, 0];
    push_u16(&mut out, byte_order, 2);
    push_u32(&mut out, byte_order, atom);
    out
}

fn change_window_attributes_request(byte_order: XByteOrder, window: u32) -> Vec<u8> {
    let mut out = vec![2, 0];
    push_u16(&mut out, byte_order, 3);
    push_u32(&mut out, byte_order, window);
    push_u32(&mut out, byte_order, 0);
    out
}

fn set_selection_owner_request(
    byte_order: XByteOrder,
    owner: u32,
    selection: u32,
    timestamp: u32,
) -> Vec<u8> {
    let mut out = vec![22, 0];
    push_u16(&mut out, byte_order, 4);
    push_u32(&mut out, byte_order, owner);
    push_u32(&mut out, byte_order, selection);
    push_u32(&mut out, byte_order, timestamp);
    out
}

fn convert_selection_request(
    byte_order: XByteOrder,
    requestor: u32,
    selection: u32,
    target: u32,
    property: u32,
    timestamp: u32,
) -> Vec<u8> {
    let mut out = vec![24, 0];
    push_u16(&mut out, byte_order, 6);
    push_u32(&mut out, byte_order, requestor);
    push_u32(&mut out, byte_order, selection);
    push_u32(&mut out, byte_order, target);
    push_u32(&mut out, byte_order, property);
    push_u32(&mut out, byte_order, timestamp);
    out
}

fn send_selection_notify_request(
    byte_order: XByteOrder,
    requestor: u32,
    timestamp: u32,
    selection: u32,
    target: u32,
    property: u32,
) -> Vec<u8> {
    let mut out = vec![25, 0];
    push_u16(&mut out, byte_order, 11);
    push_u32(&mut out, byte_order, requestor);
    push_u32(&mut out, byte_order, 0);
    out.push(31);
    out.push(0);
    push_u16(&mut out, byte_order, 0);
    push_u32(&mut out, byte_order, timestamp);
    push_u32(&mut out, byte_order, requestor);
    push_u32(&mut out, byte_order, selection);
    push_u32(&mut out, byte_order, target);
    push_u32(&mut out, byte_order, property);
    out.extend_from_slice(&[0; 8]);
    out
}

fn grab_button_request(
    byte_order: XByteOrder,
    window: u32,
    event_mask: u16,
    button: u8,
    modifiers: u16,
) -> Vec<u8> {
    let mut out = vec![28, 1];
    push_u16(&mut out, byte_order, 6);
    push_u32(&mut out, byte_order, window);
    push_u16(&mut out, byte_order, event_mask);
    out.push(1);
    out.push(1);
    push_u32(&mut out, byte_order, 0);
    push_u32(&mut out, byte_order, 0);
    out.push(button);
    out.push(0);
    push_u16(&mut out, byte_order, modifiers);
    out
}

fn ungrab_button_request(
    byte_order: XByteOrder,
    window: u32,
    button: u8,
    modifiers: u16,
) -> Vec<u8> {
    let mut out = vec![29, button];
    push_u16(&mut out, byte_order, 3);
    push_u32(&mut out, byte_order, window);
    push_u16(&mut out, byte_order, modifiers);
    push_u16(&mut out, byte_order, 0);
    out
}

fn change_property_request(
    byte_order: XByteOrder,
    mode: XPropertyMode,
    window: u32,
    property: u32,
    property_type: u32,
    format: u8,
    bytes: &[u8],
) -> Vec<u8> {
    let mode = match mode {
        XPropertyMode::Replace => 0,
        XPropertyMode::Prepend => 1,
        XPropertyMode::Append => 2,
    };
    let mut out = vec![18, mode];
    let len_units = (24 + padded_len_for_test(bytes.len())) / 4;
    push_u16(&mut out, byte_order, len_units as u16);
    push_u32(&mut out, byte_order, window);
    push_u32(&mut out, byte_order, property);
    push_u32(&mut out, byte_order, property_type);
    out.push(format);
    out.extend_from_slice(&[0, 0, 0]);
    push_u32(&mut out, byte_order, bytes.len() as u32);
    out.extend_from_slice(bytes);
    pad_to_four(&mut out);
    out
}

fn get_property_request(
    byte_order: XByteOrder,
    delete: bool,
    window: u32,
    property: u32,
    property_type: u32,
    long_offset: u32,
    long_length: u32,
) -> Vec<u8> {
    let mut out = vec![20, u8::from(delete)];
    push_u16(&mut out, byte_order, 6);
    push_u32(&mut out, byte_order, window);
    push_u32(&mut out, byte_order, property);
    push_u32(&mut out, byte_order, property_type);
    push_u32(&mut out, byte_order, long_offset);
    push_u32(&mut out, byte_order, long_length);
    out
}

fn create_gc_request(byte_order: XByteOrder, gc: u32, drawable: u32) -> Vec<u8> {
    let mut out = vec![55, 0];
    push_u16(&mut out, byte_order, 4);
    push_u32(&mut out, byte_order, gc);
    push_u32(&mut out, byte_order, drawable);
    push_u32(&mut out, byte_order, 0);
    out
}

#[allow(clippy::too_many_arguments)]
fn create_gc_values_request(
    byte_order: XByteOrder,
    gc: u32,
    drawable: u32,
    function: u32,
    plane_mask: u32,
    foreground: u32,
    background: u32,
    line_width: u32,
    font: u32,
) -> Vec<u8> {
    let value_mask = (1 << 0) | (1 << 1) | (1 << 2) | (1 << 3) | (1 << 4) | (1 << 14);
    let mut out = vec![55, 0];
    push_u16(&mut out, byte_order, 10);
    push_u32(&mut out, byte_order, gc);
    push_u32(&mut out, byte_order, drawable);
    push_u32(&mut out, byte_order, value_mask);
    for value in [
        function, plane_mask, foreground, background, line_width, font,
    ] {
        push_u32(&mut out, byte_order, value);
    }
    out
}

fn set_clip_rectangles_request(
    byte_order: XByteOrder,
    gc: u32,
    rectangles: &[(i16, i16, u16, u16)],
) -> Vec<u8> {
    let mut out = vec![59, 0];
    push_u16(&mut out, byte_order, 3 + (rectangles.len() as u16 * 2));
    push_u32(&mut out, byte_order, gc);
    push_i16(&mut out, byte_order, 0);
    push_i16(&mut out, byte_order, 0);
    for &(x, y, width, height) in rectangles {
        push_i16(&mut out, byte_order, x);
        push_i16(&mut out, byte_order, y);
        push_u16(&mut out, byte_order, width);
        push_u16(&mut out, byte_order, height);
    }
    out
}

fn change_window_event_mask_request(
    byte_order: XByteOrder,
    window: u32,
    event_mask: u32,
) -> Vec<u8> {
    let mut out = vec![2, 0];
    push_u16(&mut out, byte_order, 4);
    push_u32(&mut out, byte_order, window);
    push_u32(&mut out, byte_order, 1 << 11);
    push_u32(&mut out, byte_order, event_mask);
    out
}

fn create_pixmap_request(
    byte_order: XByteOrder,
    depth: u8,
    pixmap: u32,
    drawable: u32,
    width: u16,
    height: u16,
) -> Vec<u8> {
    let mut out = vec![53, depth];
    push_u16(&mut out, byte_order, 4);
    push_u32(&mut out, byte_order, pixmap);
    push_u32(&mut out, byte_order, drawable);
    push_u16(&mut out, byte_order, width);
    push_u16(&mut out, byte_order, height);
    out
}

fn clear_area_request(
    byte_order: XByteOrder,
    exposures: bool,
    window: u32,
    x: i16,
    y: i16,
    width: u16,
    height: u16,
) -> Vec<u8> {
    let mut out = vec![61, u8::from(exposures)];
    push_u16(&mut out, byte_order, 4);
    push_u32(&mut out, byte_order, window);
    push_i16(&mut out, byte_order, x);
    push_i16(&mut out, byte_order, y);
    push_u16(&mut out, byte_order, width);
    push_u16(&mut out, byte_order, height);
    out
}

fn open_font_request(byte_order: XByteOrder, font: u32, name: &str) -> Vec<u8> {
    let mut out = vec![45, 0];
    let len_units = (12 + padded_len_for_test(name.len())) / 4;
    push_u16(&mut out, byte_order, len_units as u16);
    push_u32(&mut out, byte_order, font);
    push_u16(&mut out, byte_order, name.len() as u16);
    push_u16(&mut out, byte_order, 0);
    out.extend_from_slice(name.as_bytes());
    pad_to_four(&mut out);
    out
}

fn list_fonts_request(byte_order: XByteOrder, max_names: u16, pattern: &str) -> Vec<u8> {
    let mut out = vec![49, 0];
    let len_units = (8 + padded_len_for_test(pattern.len())) / 4;
    push_u16(&mut out, byte_order, len_units as u16);
    push_u16(&mut out, byte_order, max_names);
    push_u16(&mut out, byte_order, pattern.len() as u16);
    out.extend_from_slice(pattern.as_bytes());
    pad_to_four(&mut out);
    out
}

fn list_fonts_with_info_request(byte_order: XByteOrder, max_names: u16, pattern: &str) -> Vec<u8> {
    let mut out = vec![50, 0];
    let len_units = (8 + padded_len_for_test(pattern.len())) / 4;
    push_u16(&mut out, byte_order, len_units as u16);
    push_u16(&mut out, byte_order, max_names);
    push_u16(&mut out, byte_order, pattern.len() as u16);
    out.extend_from_slice(pattern.as_bytes());
    pad_to_four(&mut out);
    out
}

fn create_glyph_cursor_request(
    byte_order: XByteOrder,
    cursor: u32,
    source_font: u32,
    mask_font: u32,
) -> Vec<u8> {
    let mut out = vec![94, 0];
    push_u16(&mut out, byte_order, 8);
    push_u32(&mut out, byte_order, cursor);
    push_u32(&mut out, byte_order, source_font);
    push_u32(&mut out, byte_order, mask_font);
    push_u16(&mut out, byte_order, 1);
    push_u16(&mut out, byte_order, 2);
    push_u16(&mut out, byte_order, u16::MAX);
    push_u16(&mut out, byte_order, u16::MAX);
    push_u16(&mut out, byte_order, u16::MAX);
    push_u16(&mut out, byte_order, 0);
    push_u16(&mut out, byte_order, 0);
    push_u16(&mut out, byte_order, 0);
    out
}

fn recolor_cursor_request(byte_order: XByteOrder, cursor: u32) -> Vec<u8> {
    let mut out = vec![96, 0];
    push_u16(&mut out, byte_order, 5);
    push_u32(&mut out, byte_order, cursor);
    for value in [u16::MAX, u16::MAX, u16::MAX, 0, 0, 0] {
        push_u16(&mut out, byte_order, value);
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn copy_area_request(
    byte_order: XByteOrder,
    source: u32,
    destination: u32,
    gc: u32,
    src_x: i16,
    src_y: i16,
    dst_x: i16,
    dst_y: i16,
    width: u16,
    height: u16,
) -> Vec<u8> {
    let mut out = vec![62, 0];
    push_u16(&mut out, byte_order, 7);
    push_u32(&mut out, byte_order, source);
    push_u32(&mut out, byte_order, destination);
    push_u32(&mut out, byte_order, gc);
    push_i16(&mut out, byte_order, src_x);
    push_i16(&mut out, byte_order, src_y);
    push_i16(&mut out, byte_order, dst_x);
    push_i16(&mut out, byte_order, dst_y);
    push_u16(&mut out, byte_order, width);
    push_u16(&mut out, byte_order, height);
    out
}

fn poly_fill_rectangle_request(
    byte_order: XByteOrder,
    drawable: u32,
    gc: u32,
    rectangles: &[(i16, i16, u16, u16)],
) -> Vec<u8> {
    let mut out = vec![70, 0];
    let len_units = 3 + rectangles.len() * 2;
    push_u16(&mut out, byte_order, len_units as u16);
    push_u32(&mut out, byte_order, drawable);
    push_u32(&mut out, byte_order, gc);
    for (x, y, width, height) in rectangles {
        push_i16(&mut out, byte_order, *x);
        push_i16(&mut out, byte_order, *y);
        push_u16(&mut out, byte_order, *width);
        push_u16(&mut out, byte_order, *height);
    }
    out
}

fn poly_text8_request(
    byte_order: XByteOrder,
    drawable: u32,
    gc: u32,
    x: i16,
    y: i16,
    text: &[u8],
) -> Vec<u8> {
    let mut out = vec![74, 0];
    let len_units = padded_len_for_test(18 + text.len()) / 4;
    push_u16(&mut out, byte_order, len_units as u16);
    push_u32(&mut out, byte_order, drawable);
    push_u32(&mut out, byte_order, gc);
    push_i16(&mut out, byte_order, x);
    push_i16(&mut out, byte_order, y);
    out.push(u8::try_from(text.len()).unwrap());
    out.push(0);
    out.extend_from_slice(text);
    pad_to_four(&mut out);
    out
}

fn image_text8_request(
    byte_order: XByteOrder,
    drawable: u32,
    gc: u32,
    x: i16,
    y: i16,
    text: &[u8],
) -> Vec<u8> {
    let mut out = vec![76, u8::try_from(text.len()).unwrap()];
    let len_units = (16 + padded_len_for_test(text.len())) / 4;
    push_u16(&mut out, byte_order, len_units as u16);
    push_u32(&mut out, byte_order, drawable);
    push_u32(&mut out, byte_order, gc);
    push_i16(&mut out, byte_order, x);
    push_i16(&mut out, byte_order, y);
    out.extend_from_slice(text);
    pad_to_four(&mut out);
    out
}

fn poly_text8_compact_item_request(
    byte_order: XByteOrder,
    drawable: u32,
    gc: u32,
    x: i16,
    y: i16,
    text: &[u8],
) -> Vec<u8> {
    let mut out = vec![74, 0];
    let len_units = padded_len_for_test(18 + text.len()) / 4;
    push_u16(&mut out, byte_order, len_units as u16);
    push_u32(&mut out, byte_order, drawable);
    push_u32(&mut out, byte_order, gc);
    push_i16(&mut out, byte_order, x);
    push_i16(&mut out, byte_order, y);
    out.push(u8::try_from(text.len() + 1).unwrap());
    out.push(0);
    out.extend_from_slice(text);
    pad_to_four(&mut out);
    out
}

fn poly_segment_request(
    byte_order: XByteOrder,
    drawable: u32,
    gc: u32,
    segments: &[(i16, i16, i16, i16)],
) -> Vec<u8> {
    let mut out = vec![66, 0];
    let len_units = 3 + segments.len() * 2;
    push_u16(&mut out, byte_order, len_units as u16);
    push_u32(&mut out, byte_order, drawable);
    push_u32(&mut out, byte_order, gc);
    for (x1, y1, x2, y2) in segments {
        push_i16(&mut out, byte_order, *x1);
        push_i16(&mut out, byte_order, *y1);
        push_i16(&mut out, byte_order, *x2);
        push_i16(&mut out, byte_order, *y2);
    }
    out
}

fn poly_line_request(
    byte_order: XByteOrder,
    drawable: u32,
    gc: u32,
    points: &[(i16, i16)],
) -> Vec<u8> {
    let mut out = vec![65, 0];
    let len_units = 3 + points.len();
    push_u16(&mut out, byte_order, len_units as u16);
    push_u32(&mut out, byte_order, drawable);
    push_u32(&mut out, byte_order, gc);
    for (x, y) in points {
        push_i16(&mut out, byte_order, *x);
        push_i16(&mut out, byte_order, *y);
    }
    out
}

fn poly_fill_arc_request(
    byte_order: XByteOrder,
    drawable: u32,
    gc: u32,
    arcs: &[(i16, i16, u16, u16, i16, i16)],
) -> Vec<u8> {
    let mut out = vec![71, 0];
    let len_units = 3 + arcs.len() * 3;
    push_u16(&mut out, byte_order, len_units as u16);
    push_u32(&mut out, byte_order, drawable);
    push_u32(&mut out, byte_order, gc);
    for (x, y, width, height, angle1, angle2) in arcs {
        push_i16(&mut out, byte_order, *x);
        push_i16(&mut out, byte_order, *y);
        push_u16(&mut out, byte_order, *width);
        push_u16(&mut out, byte_order, *height);
        push_i16(&mut out, byte_order, *angle1);
        push_i16(&mut out, byte_order, *angle2);
    }
    out
}

fn fill_poly_request(
    byte_order: XByteOrder,
    drawable: u32,
    gc: u32,
    points: &[(i16, i16)],
) -> Vec<u8> {
    let mut out = vec![69, 0];
    let len_units = 4 + points.len();
    push_u16(&mut out, byte_order, len_units as u16);
    push_u32(&mut out, byte_order, drawable);
    push_u32(&mut out, byte_order, gc);
    out.extend_from_slice(&[0, 0, 0, 0]);
    for (x, y) in points {
        push_i16(&mut out, byte_order, *x);
        push_i16(&mut out, byte_order, *y);
    }
    out
}

fn put_image_request(
    byte_order: XByteOrder,
    drawable: u32,
    gc: u32,
    width: u16,
    height: u16,
    dst_x: i16,
    dst_y: i16,
    data: &[u8],
) -> Vec<u8> {
    let mut out = vec![72, 2];
    let len_units = (24 + padded_len_for_test(data.len())) / 4;
    push_u16(&mut out, byte_order, len_units as u16);
    push_u32(&mut out, byte_order, drawable);
    push_u32(&mut out, byte_order, gc);
    push_u16(&mut out, byte_order, width);
    push_u16(&mut out, byte_order, height);
    push_i16(&mut out, byte_order, dst_x);
    push_i16(&mut out, byte_order, dst_y);
    out.push(0);
    out.push(24);
    push_u16(&mut out, byte_order, 0);
    out.extend_from_slice(data);
    pad_to_four(&mut out);
    out
}

fn sophia_present_pixmap_request(
    byte_order: XByteOrder,
    window: u32,
    pixmap: u32,
    damage: (i16, i16, u16, u16),
    previous_committed_generation: u64,
    timeout_msec: u32,
) -> Vec<u8> {
    let mut out = vec![
        X_SOPHIA_PRESENT_MAJOR_OPCODE,
        X_SOPHIA_PRESENT_PIXMAP_MINOR_OPCODE,
    ];
    push_u16(&mut out, byte_order, 8);
    push_u32(&mut out, byte_order, window);
    push_u32(&mut out, byte_order, pixmap);
    push_i16(&mut out, byte_order, damage.0);
    push_i16(&mut out, byte_order, damage.1);
    push_u16(&mut out, byte_order, damage.2);
    push_u16(&mut out, byte_order, damage.3);
    push_u64(&mut out, byte_order, previous_committed_generation);
    push_u32(&mut out, byte_order, timeout_msec);
    out
}

fn mit_shm_query_version_request(byte_order: XByteOrder) -> Vec<u8> {
    let mut out = vec![X_MIT_SHM_MAJOR_OPCODE, X_MIT_SHM_QUERY_VERSION_MINOR_OPCODE];
    push_u16(&mut out, byte_order, 1);
    out
}

fn mit_shm_attach_request(
    byte_order: XByteOrder,
    segment: u32,
    shmid: u32,
    read_only: bool,
) -> Vec<u8> {
    let mut out = vec![X_MIT_SHM_MAJOR_OPCODE, X_MIT_SHM_ATTACH_MINOR_OPCODE];
    push_u16(&mut out, byte_order, 4);
    push_u32(&mut out, byte_order, segment);
    push_u32(&mut out, byte_order, shmid);
    out.push(u8::from(read_only));
    out.extend_from_slice(&[0, 0, 0]);
    out
}

fn mit_shm_detach_request(byte_order: XByteOrder, segment: u32) -> Vec<u8> {
    let mut out = vec![X_MIT_SHM_MAJOR_OPCODE, X_MIT_SHM_DETACH_MINOR_OPCODE];
    push_u16(&mut out, byte_order, 2);
    push_u32(&mut out, byte_order, segment);
    out
}

fn mit_shm_put_image_request(
    byte_order: XByteOrder,
    drawable: u32,
    gc: u32,
    segment: u32,
    offset: u32,
) -> Vec<u8> {
    let mut out = vec![X_MIT_SHM_MAJOR_OPCODE, X_MIT_SHM_PUT_IMAGE_MINOR_OPCODE];
    push_u16(&mut out, byte_order, 10);
    push_u32(&mut out, byte_order, drawable);
    push_u32(&mut out, byte_order, gc);
    push_u16(&mut out, byte_order, 64);
    push_u16(&mut out, byte_order, 48);
    push_u16(&mut out, byte_order, 0);
    push_u16(&mut out, byte_order, 0);
    push_u16(&mut out, byte_order, 32);
    push_u16(&mut out, byte_order, 24);
    push_i16(&mut out, byte_order, 3);
    push_i16(&mut out, byte_order, 5);
    out.push(24);
    out.push(2);
    out.push(0);
    out.push(0);
    push_u32(&mut out, byte_order, segment);
    push_u32(&mut out, byte_order, offset);
    out
}

fn randr_query_version_request(
    byte_order: XByteOrder,
    major_version: u32,
    minor_version: u32,
) -> Vec<u8> {
    let mut out = vec![X_RANDR_MAJOR_OPCODE, X_RANDR_QUERY_VERSION_MINOR_OPCODE];
    push_u16(&mut out, byte_order, 3);
    push_u32(&mut out, byte_order, major_version);
    push_u32(&mut out, byte_order, minor_version);
    out
}

fn extension_query_version_request(
    byte_order: XByteOrder,
    opcode: u8,
    major_version: u32,
    minor_version: u32,
) -> Vec<u8> {
    let mut out = vec![opcode, 0];
    push_u16(&mut out, byte_order, 3);
    push_u32(&mut out, byte_order, major_version);
    push_u32(&mut out, byte_order, minor_version);
    out
}

fn dri3_open_request(byte_order: XByteOrder, drawable: u32, provider: u32) -> Vec<u8> {
    let mut out = vec![X_DRI3_MAJOR_OPCODE, X_DRI3_OPEN_MINOR_OPCODE];
    push_u16(&mut out, byte_order, 3);
    push_u32(&mut out, byte_order, drawable);
    push_u32(&mut out, byte_order, provider);
    out
}

fn dri3_get_supported_modifiers_request(
    byte_order: XByteOrder,
    window: u32,
    depth: u8,
    bits_per_pixel: u8,
) -> Vec<u8> {
    let mut out = vec![
        X_DRI3_MAJOR_OPCODE,
        X_DRI3_GET_SUPPORTED_MODIFIERS_MINOR_OPCODE,
    ];
    push_u16(&mut out, byte_order, 3);
    push_u32(&mut out, byte_order, window);
    out.push(depth);
    out.push(bits_per_pixel);
    out.extend_from_slice(&[0; 2]);
    out
}

#[allow(clippy::too_many_arguments)]
fn dri3_pixmap_from_buffer_request(
    byte_order: XByteOrder,
    pixmap: u32,
    drawable: u32,
    size_bytes: u32,
    width: u16,
    height: u16,
    stride: u16,
    depth: u8,
    bits_per_pixel: u8,
) -> Vec<u8> {
    let mut out = vec![X_DRI3_MAJOR_OPCODE, X_DRI3_PIXMAP_FROM_BUFFER_MINOR_OPCODE];
    push_u16(&mut out, byte_order, 6);
    push_u32(&mut out, byte_order, pixmap);
    push_u32(&mut out, byte_order, drawable);
    push_u32(&mut out, byte_order, size_bytes);
    push_u16(&mut out, byte_order, width);
    push_u16(&mut out, byte_order, height);
    push_u16(&mut out, byte_order, stride);
    out.push(depth);
    out.push(bits_per_pixel);
    out
}

#[allow(clippy::too_many_arguments)]
fn dri3_pixmap_from_buffers_request(
    byte_order: XByteOrder,
    pixmap: u32,
    window: u32,
    num_buffers: u8,
    width: u16,
    height: u16,
    strides: [u32; sophia_protocol::DMA_BUF_MAX_PLANES],
    offsets: [u32; sophia_protocol::DMA_BUF_MAX_PLANES],
    depth: u8,
    bits_per_pixel: u8,
    modifier: u64,
) -> Vec<u8> {
    let mut out = vec![X_DRI3_MAJOR_OPCODE, X_DRI3_PIXMAP_FROM_BUFFERS_MINOR_OPCODE];
    push_u16(&mut out, byte_order, 16);
    push_u32(&mut out, byte_order, pixmap);
    push_u32(&mut out, byte_order, window);
    out.push(num_buffers);
    out.extend_from_slice(&[0; 3]);
    push_u16(&mut out, byte_order, width);
    push_u16(&mut out, byte_order, height);
    for (stride, offset) in strides.into_iter().zip(offsets) {
        push_u32(&mut out, byte_order, stride);
        push_u32(&mut out, byte_order, offset);
    }
    out.push(depth);
    out.push(bits_per_pixel);
    out.extend_from_slice(&[0; 2]);
    push_u64(&mut out, byte_order, modifier);
    out
}

fn dri3_fence_from_fd_request(
    byte_order: XByteOrder,
    drawable: u32,
    fence: u32,
    initially_triggered: bool,
) -> Vec<u8> {
    let mut out = vec![X_DRI3_MAJOR_OPCODE, X_DRI3_FENCE_FROM_FD_MINOR_OPCODE];
    push_u16(&mut out, byte_order, 4);
    push_u32(&mut out, byte_order, drawable);
    push_u32(&mut out, byte_order, fence);
    out.push(u8::from(initially_triggered));
    out.extend_from_slice(&[0; 3]);
    out
}

fn xfixes_create_region_request(
    byte_order: XByteOrder,
    region: u32,
    rectangles: &[Rect],
) -> Vec<u8> {
    let mut out = vec![X_XFIXES_MAJOR_OPCODE, X_XFIXES_CREATE_REGION_MINOR_OPCODE];
    push_u16(&mut out, byte_order, (2 + rectangles.len() * 2) as u16);
    push_u32(&mut out, byte_order, region);
    for rectangle in rectangles {
        push_i16(&mut out, byte_order, rectangle.x as i16);
        push_i16(&mut out, byte_order, rectangle.y as i16);
        push_u16(&mut out, byte_order, rectangle.width as u16);
        push_u16(&mut out, byte_order, rectangle.height as u16);
    }
    out
}

fn xfixes_set_region_request(byte_order: XByteOrder, region: u32, rectangles: &[Rect]) -> Vec<u8> {
    let mut out = vec![X_XFIXES_MAJOR_OPCODE, X_XFIXES_SET_REGION_MINOR_OPCODE];
    push_u16(&mut out, byte_order, (2 + rectangles.len() * 2) as u16);
    push_u32(&mut out, byte_order, region);
    for rectangle in rectangles {
        push_i16(&mut out, byte_order, rectangle.x as i16);
        push_i16(&mut out, byte_order, rectangle.y as i16);
        push_u16(&mut out, byte_order, rectangle.width as u16);
        push_u16(&mut out, byte_order, rectangle.height as u16);
    }
    out
}

fn xfixes_select_selection_input_request(
    byte_order: XByteOrder,
    window: u32,
    selection: u32,
    event_mask: u32,
) -> Vec<u8> {
    let mut out = vec![
        X_XFIXES_MAJOR_OPCODE,
        X_XFIXES_SELECT_SELECTION_INPUT_MINOR_OPCODE,
    ];
    push_u16(&mut out, byte_order, 4);
    push_u32(&mut out, byte_order, window);
    push_u32(&mut out, byte_order, selection);
    push_u32(&mut out, byte_order, event_mask);
    out
}

fn randr_get_output_property_request(
    byte_order: XByteOrder,
    output: u32,
    property: u32,
    long_length: u32,
) -> Vec<u8> {
    let mut out = vec![
        X_RANDR_MAJOR_OPCODE,
        X_RANDR_GET_OUTPUT_PROPERTY_MINOR_OPCODE,
    ];
    push_u16(&mut out, byte_order, 7);
    push_u32(&mut out, byte_order, output);
    push_u32(&mut out, byte_order, property);
    push_u32(&mut out, byte_order, 0);
    push_u32(&mut out, byte_order, 0);
    push_u32(&mut out, byte_order, long_length);
    out.extend_from_slice(&[0, 0, 0, 0]);
    out
}

fn present_pixmap_request(
    byte_order: XByteOrder,
    window: XResourceId,
    pixmap: XResourceId,
    serial: u32,
) -> Vec<u8> {
    let mut out = vec![X_PRESENT_MAJOR_OPCODE, X_PRESENT_PIXMAP_MINOR_OPCODE];
    push_u16(&mut out, byte_order, 18);
    push_u32(&mut out, byte_order, window.local.raw() as u32);
    push_u32(&mut out, byte_order, pixmap.local.raw() as u32);
    push_u32(&mut out, byte_order, serial);
    push_u32(&mut out, byte_order, 0);
    push_u32(&mut out, byte_order, 0);
    push_u16(&mut out, byte_order, 0);
    push_u16(&mut out, byte_order, 0);
    push_u32(&mut out, byte_order, 0);
    push_u32(&mut out, byte_order, 0);
    push_u32(&mut out, byte_order, 0);
    push_u32(&mut out, byte_order, 0);
    push_u32(&mut out, byte_order, 0);
    push_u64(&mut out, byte_order, 0);
    push_u64(&mut out, byte_order, 0);
    push_u64(&mut out, byte_order, 0);
    out
}

fn randr_select_input_request(byte_order: XByteOrder, window: u32, enable: u16) -> Vec<u8> {
    let mut out = vec![X_RANDR_MAJOR_OPCODE, X_RANDR_SELECT_INPUT_MINOR_OPCODE];
    push_u16(&mut out, byte_order, 3);
    push_u32(&mut out, byte_order, window);
    push_u16(&mut out, byte_order, enable);
    push_u16(&mut out, byte_order, 0);
    out
}

fn randr_get_monitors_request(byte_order: XByteOrder, window: u32, get_active: bool) -> Vec<u8> {
    let mut out = vec![X_RANDR_MAJOR_OPCODE, X_RANDR_GET_MONITORS_MINOR_OPCODE];
    push_u16(&mut out, byte_order, 3);
    push_u32(&mut out, byte_order, window);
    out.push(u8::from(get_active));
    out.extend_from_slice(&[0, 0, 0]);
    out
}

fn randr_window_request(byte_order: XByteOrder, minor_opcode: u8, window: u32) -> Vec<u8> {
    let mut out = vec![X_RANDR_MAJOR_OPCODE, minor_opcode];
    push_u16(&mut out, byte_order, 2);
    push_u32(&mut out, byte_order, window);
    out
}

fn query_extension_request(byte_order: XByteOrder, name: &str) -> Vec<u8> {
    let mut out = vec![98, 0];
    let len_units = (8 + padded_len_for_test(name.len())) / 4;
    push_u16(&mut out, byte_order, len_units as u16);
    push_u16(&mut out, byte_order, name.len() as u16);
    push_u16(&mut out, byte_order, 0);
    out.extend_from_slice(name.as_bytes());
    pad_to_four(&mut out);
    out
}

fn push_u16(out: &mut Vec<u8>, byte_order: XByteOrder, value: u16) {
    match byte_order {
        XByteOrder::LittleEndian => out.extend_from_slice(&value.to_le_bytes()),
        XByteOrder::BigEndian => out.extend_from_slice(&value.to_be_bytes()),
    }
}

fn push_i16(out: &mut Vec<u8>, byte_order: XByteOrder, value: i16) {
    match byte_order {
        XByteOrder::LittleEndian => out.extend_from_slice(&value.to_le_bytes()),
        XByteOrder::BigEndian => out.extend_from_slice(&value.to_be_bytes()),
    }
}

fn push_u32(out: &mut Vec<u8>, byte_order: XByteOrder, value: u32) {
    match byte_order {
        XByteOrder::LittleEndian => out.extend_from_slice(&value.to_le_bytes()),
        XByteOrder::BigEndian => out.extend_from_slice(&value.to_be_bytes()),
    }
}

fn push_u64(out: &mut Vec<u8>, byte_order: XByteOrder, value: u64) {
    match byte_order {
        XByteOrder::LittleEndian => out.extend_from_slice(&value.to_le_bytes()),
        XByteOrder::BigEndian => out.extend_from_slice(&value.to_be_bytes()),
    }
}

fn query_colors_request(byte_order: XByteOrder, colormap: u32, pixels: &[u32]) -> Vec<u8> {
    let mut out = vec![91, 0];
    let len_units = 2 + pixels.len();
    push_u16(&mut out, byte_order, len_units as u16);
    push_u32(&mut out, byte_order, colormap);
    for pixel in pixels {
        push_u32(&mut out, byte_order, *pixel);
    }
    out
}

fn create_colormap_request(
    byte_order: XByteOrder,
    colormap: u32,
    window: u32,
    visual: u32,
) -> Vec<u8> {
    let mut out = vec![78, 0];
    push_u16(&mut out, byte_order, 4);
    push_u32(&mut out, byte_order, colormap);
    push_u32(&mut out, byte_order, window);
    push_u32(&mut out, byte_order, visual);
    out
}

fn alloc_color_request(
    byte_order: XByteOrder,
    colormap: u32,
    red: u16,
    green: u16,
    blue: u16,
) -> Vec<u8> {
    let mut out = vec![84, 0];
    push_u16(&mut out, byte_order, 4);
    push_u32(&mut out, byte_order, colormap);
    push_u16(&mut out, byte_order, red);
    push_u16(&mut out, byte_order, green);
    push_u16(&mut out, byte_order, blue);
    push_u16(&mut out, byte_order, 0);
    out
}

fn alloc_named_color_request(byte_order: XByteOrder, colormap: u32, name: &str) -> Vec<u8> {
    let mut out = vec![85, 0];
    let len_units = (12 + padded_len_for_test(name.len())) / 4;
    push_u16(&mut out, byte_order, len_units as u16);
    push_u32(&mut out, byte_order, colormap);
    push_u16(&mut out, byte_order, name.len() as u16);
    push_u16(&mut out, byte_order, 0);
    out.extend_from_slice(name.as_bytes());
    pad_to_four(&mut out);
    out
}

fn xkb_use_extension_request(
    byte_order: XByteOrder,
    wanted_major: u16,
    wanted_minor: u16,
) -> Vec<u8> {
    let mut out = vec![
        X_KEYBOARD_MAJOR_OPCODE,
        X_KEYBOARD_USE_EXTENSION_MINOR_OPCODE,
    ];
    push_u16(&mut out, byte_order, 2);
    push_u16(&mut out, byte_order, wanted_major);
    push_u16(&mut out, byte_order, wanted_minor);
    out
}

fn configure_window_request(
    byte_order: XByteOrder,
    window: u32,
    value_mask: u16,
    values: &[u32],
) -> Vec<u8> {
    let mut out = vec![12, 0];
    let len_units = 3 + values.len();
    push_u16(&mut out, byte_order, len_units as u16);
    push_u32(&mut out, byte_order, window);
    push_u16(&mut out, byte_order, value_mask);
    push_u16(&mut out, byte_order, 0);
    for value in values {
        push_u32(&mut out, byte_order, *value);
    }
    out
}

fn read_u16(byte_order: XByteOrder, bytes: &[u8]) -> u16 {
    match byte_order {
        XByteOrder::LittleEndian => u16::from_le_bytes(bytes.try_into().unwrap()),
        XByteOrder::BigEndian => u16::from_be_bytes(bytes.try_into().unwrap()),
    }
}

fn read_i16(byte_order: XByteOrder, bytes: &[u8]) -> i16 {
    match byte_order {
        XByteOrder::LittleEndian => i16::from_le_bytes(bytes.try_into().unwrap()),
        XByteOrder::BigEndian => i16::from_be_bytes(bytes.try_into().unwrap()),
    }
}

fn read_u32(byte_order: XByteOrder, bytes: &[u8]) -> u32 {
    match byte_order {
        XByteOrder::LittleEndian => u32::from_le_bytes(bytes.try_into().unwrap()),
        XByteOrder::BigEndian => u32::from_be_bytes(bytes.try_into().unwrap()),
    }
}

fn read_u64(byte_order: XByteOrder, bytes: &[u8]) -> u64 {
    match byte_order {
        XByteOrder::LittleEndian => u64::from_le_bytes(bytes.try_into().unwrap()),
        XByteOrder::BigEndian => u64::from_be_bytes(bytes.try_into().unwrap()),
    }
}

fn pad_to_four(out: &mut Vec<u8>) {
    out.resize(padded_len_for_test(out.len()), 0);
}

const fn padded_len_for_test(len: usize) -> usize {
    (len + 3) & !3
}

#[cfg(unix)]
fn wait_for_socket(path: &std::path::Path) {
    for _ in 0..100 {
        if path.exists() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    panic!("timed out waiting for socket {}", path.display());
}

#[cfg(unix)]
fn read_setup_success(stream: &mut std::os::unix::net::UnixStream, byte_order: XByteOrder) {
    let _ = read_setup_resource_id_base(stream, byte_order);
}

#[cfg(unix)]
fn read_setup_resource_id_base(
    stream: &mut std::os::unix::net::UnixStream,
    byte_order: XByteOrder,
) -> u32 {
    use std::io::Read;

    let mut prefix = [0; X_SETUP_REPLY_PREFIX_LEN];
    stream.read_exact(&mut prefix).unwrap();
    assert_eq!(prefix[0], 1);
    let body_len = usize::from(read_u16(byte_order, &prefix[6..8])) * 4;
    let mut body = vec![0; body_len];
    stream.read_exact(&mut body).unwrap();
    read_u32(byte_order, &body[4..8])
}

#[cfg(unix)]
fn read_x_record(stream: &mut std::os::unix::net::UnixStream) -> [u8; 32] {
    use std::io::Read;

    let mut record = [0; 32];
    stream.read_exact(&mut record).unwrap();
    record
}

#[cfg(unix)]
fn read_x_reply(stream: &mut std::os::unix::net::UnixStream, byte_order: XByteOrder) -> Vec<u8> {
    use std::io::Read;

    let mut prefix = [0; 32];
    stream.read_exact(&mut prefix).unwrap();
    let body_len = usize::try_from(read_u32(byte_order, &prefix[4..8])).unwrap() * 4;
    let mut reply = prefix.to_vec();
    reply.resize(32 + body_len, 0);
    stream.read_exact(&mut reply[32..]).unwrap();
    reply
}
